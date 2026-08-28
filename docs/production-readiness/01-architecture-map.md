# mmorch — Mapa de arquitectura real (production-readiness 01)

Fecha: 2026-08-27. Fuente: lectura directa del repo `~/.claude/orchestration`
(paquete `mmorch`, 130 módulos en `mmorch/`, ~22.7k LOC en el paquete, 101 archivos de test).
Hechos con `file:line`; sin ediciones de código.

---

## 1. Capas reales (como están, no como se describen)

```
┌─────────────────────────────────────────────────────────────────┐
│ ENTRY POINTS                                                    │
│  mcp_server.py (stdio FastMCP, ~44 tools)                       │
│  mmorch.server (HTTP Starlette+SSE, jobs in-process)            │
│  scripts/nightly.py, scripts/autopull.cmd (batch/scheduled)     │
│  import mmorch (API de librería, __init__ re-exporta ~48 mods)  │
│  CLI: mmorch-server, mmorch-sync (pyproject.toml:33-35)         │
├─────────────────────────────────────────────────────────────────┤
│ PATRONES DE ORQUESTACIÓN (capa de dominio)                      │
│  patterns.py (fan_out, adversarial_verify) · route.py ·         │
│  cascade.py · ensemble.py · tournament.py · bucketrank.py ·     │
│  classify.py · spec.py · rubric_loop.py · hillclimb.py ·        │
│  autoresearch.py · code_loop.py · project_build/loop/           │
│  integrate/repair (motor /project F1-F4) · evolve.py            │
├─────────────────────────────────────────────────────────────────┤
│ APRENDIZAJE / MEMORIA (capa cognitiva)                          │
│  feedback.py (outcomes + ThompsonBandit) · intuition.py         │
│  (sig-bandit) · memory.py (DuckDB 2 capas) · retention.py ·     │
│  curiosity.py · shadow_prior.py · predict.py · factory.py       │
├─────────────────────────────────────────────────────────────────┤
│ NODOS / PROVIDERS (capa de infraestructura)                     │
│  config.py (REGISTRY de modelos) · providers.py (call único     │
│  OpenAI-compatible) · cost.py · prices.py · budget.py ·         │
│  metrics.py (JSONL append-only) · cache.py · events.py (bus)    │
├─────────────────────────────────────────────────────────────────┤
│ ESTADO PERSISTENTE (sin capa: archivos sueltos)                 │
│  logs/ = 69 archivos, 58MB (JSONL + JSON + DuckDB + .out/.bak)  │
│  raíz  = chat.db, workflow.db, prices.json, projects.json       │
└─────────────────────────────────────────────────────────────────┘
```

La capa de patrones es genuinamente limpia: cada patrón es un módulo con una función
pública y un dataclass de resultado, todos convergen en `providers.call()`
(`providers.py:104`) que es el único punto de salida a APIs externas y el único punto
de logging de métricas. Eso es un módulo profundo real: budget-gate + client cache +
clasificación de errores + costo + telemetría detrás de una firma.

## 2. Entry points

| Entry | Archivo | Naturaleza |
|---|---|---|
| MCP server | `mcp_server.py` (856 líneas, ~44 `@mcp.tool()`) | stdio FastMCP; wrapper fino: cada tool = import + llamada + `json.dumps` a mano |
| HTTP server | `mmorch/server.py:1-21` (858 líneas) + `server_engine.py` (411) + `server_core.py` (43) + `server_frontend.py`/`server_pty.py`/`server_fleet.py` | Starlette+SSE; corre jobs **in-process** en threads (`server_engine._rubric_drive`, `server_engine.py:17`) |
| Librería | `mmorch/__init__.py` | re-exporta ~48 módulos hermanos **eagerly** (48 `from .x import`, medido) |
| Batch | `scripts/nightly.py`, `scripts/autopull.cmd`, `scripts/session_ingest_hook.py` | procesos separados que escriben el MISMO estado (bandit, metrics, memoria) |
| CLI scripts | `[project.scripts]` `mmorch-server`, `mmorch-sync` (pyproject.toml:33-35) | únicos entry points instalables |

Telemetría MCP: `mcp_server.py:55-57` instrumenta el server (`mcp_telemetry.instrument`)
→ `logs/mcp_calls.jsonl` registra CADA tool call.

## 3. Flujo de datos canónico (request → … → memoria/metrics)

1. **Request** entra por MCP tool o llamada de librería (ej. `mmorch_route`, `mcp_server.py:162`).
2. **Router**: `route.route()` → opcional `intuition.decide()` (bandit por firma
   estructural, `intuition.py:34-37`, estado `logs/bandit_sig.json`) elige modelo si la
   firma es familiar; si no, `DEFAULT_GENERATOR` (`config.py:160`).
3. **Nodo modelo**: `providers.call()` (`providers.py:104`):
   - gate de presupuesto ANTES de la call (`providers.py:129-139` → `budget.check()`,
     que suma el mes desde `metrics.jsonl`, `budget.py:45`);
   - client OpenAI-compatible cacheado por `provider:base_url` (`providers.py:88-101`);
   - éxito o error, SIEMPRE `metrics.log_event()` → `logs/metrics.jsonl`
     (`providers.py:161-196`), con `error_class` (`_classify_error`, `providers.py:35`)
     y `cached_tokens` (`providers.py:53`).
4. **Verificador**: `patterns.adversarial_verify` / `ensemble.ensemble_verify` con regla
   cross-family enforced (`config.family_of`, `config.py:169`; task-aware en
   `mcp_server.py:87-121`).
5. **Outcome / aprendizaje**: `feedback.record_outcome()` (`feedback.py:42`) apendea
   `logs/feedback.jsonl` y, si hay `context`, entrena el sig-bandit
   (`feedback.py:56-61`, lazy-import de intuition para romper el ciclo). El handler MCP
   `mmorch_record_outcome` ADEMÁS actualiza el bandit plano `bandit_state.json`
   (`mcp_server.py:585-587`).
6. **Memoria**: `memory.remember` → episódico inmutable + nota destilada + embedding
   local fastembed (`memory.py:21-23`, degrade graceful sin fastembed) en
   `logs/memory.duckdb` (11MB, `memory.py:33`).
7. **UI live**: `events.emit()` (ring buffer in-process, `events.py:1-9`) → SSE del
   server; el JSONL queda como audit durable.

## 4. Config, env vars, keys

- **Registry de modelos**: `config.py:29-157` — 9 modelos, 4 familias (deepseek, google,
  moonshot, zhipu), precios hardcodeados "jun-2026, VOLATILE" (`config.py:3-4`);
  override por `prices.py`/`prices.json`. Defaults: `config.py:160-166`.
- **Keys**: env vars por modelo (`api_key_env`), cargadas por `load_dotenv()` **al
  importar** `providers.py:19-22` (side effect de import: lee
  `~/.claude/orchestration/.env`). `MissingKeyError` con mensaje claro (`providers.py:92-97`).
- **Env vars operativas**: `MMORCH_MAX_MONTHLY_USD` (budget, opt-in, sin default →
  ilimitado, `budget.py:34-40`), `MMORCH_SERVER_TOKEN` (vacío = **sin auth**,
  `server_core.py:20-25`), `MMORCH_SERVER_HOST` (default 127.0.0.1).
- Gates de calidad: ruff (F/E9/B/PLE) + mypy en 0, enforced (pyproject.toml:37-56).

## 5. Estado persistente (inventario real)

| Store | Dónde | Qué |
|---|---|---|
| `logs/metrics.jsonl` | 5.4MB, 18.5k líneas | telemetría por call; **input del budget** — sin rotación |
| `logs/memory.duckdb` | 11MB | episódico + semántico + embeddings 384d (`memory.py:105-120`) |
| `logs/feedback.jsonl` | 655KB | outcomes etiquetados |
| `logs/bandit_state.json` + `logs/bandit_sig.json` | 168B / 9.7KB | DOS bandits separados (plano vs firma) |
| `logs/*` restantes | 69 archivos, 58MB total | mezcla de: estado aprendido (`workflow_bandit.json`), datasets (`codequality_dataset.jsonl` 11MB), caches (`worklist_cache.json`), logs de proceso (`autopull.log` 4.4MB), backups a mano (`*.bak`), outputs sueltos (`*.out`) |
| raíz del repo | `chat.db` (16KB), `workflow.db` (147KB), `prices.json`, `projects.json` | estado FUERA de logs/, convención rota |

Todas las rutas de estado derivan de `ROOT = Path(__file__).resolve().parent.parent`
— patrón repetido en **27 módulos** (medido por grep; 17 con `ROOT =` propio), ej.
`memory.py:32`, `feedback.py:26`, `intuition.py:30`, `metrics.py:22`.

## 6. Evaluación de forma / lógica

### Lo profundo (bien)
- `providers.call()` — un solo punto de salida con budget/telemetría/error-class detrás
  de una firma chica (`providers.py:104-205`).
- `project_build.py` — validador de plan y stub-detector **deterministas** con planner
  LLM inyectable (`project_build.py:1-14`); allowlist de `test_cmd` contra
  prompt-injection→RCE (`project_build.py:24-40`). Seam de test real.
- `memory.py` — dos capas con degrade graceful sin fastembed (`memory.py:17-24`),
  `_connect(path=...)` inyectable para tests (`memory.py:105`).
- `feedback.py`/`intuition.py` — el sig-bandit envuelve el bandit existente en vez de
  duplicar el learner (`intuition.py:3-6`); write atómico del estado (`feedback.py:125-127`).
- Import graph: fan-in alto concentrado en la base correcta (config 42, providers 28,
  feedback 18) — dependencia hacia abajo, casi sin ciclos (el único ciclo
  feedback↔intuition está roto por lazy import comentado, `feedback.py:53-56`).

### Lo shallow / la deuda
- `mcp_server.py`: 44 wrappers que repiten el mismo boilerplate
  (import-alias → call → dict → `json.dumps`) + docstrings-manual de ~15 líneas cada
  uno; la lógica real (ej. strip de code-fences en `mmorch_evolve_self`,
  `mcp_server.py:630-635`; el bridge/babel-thread de `mmorch_vault_write`,
  `mcp_server.py:374-401`) vive EN el wrapper, no en la librería → la vía librería y la
  vía MCP divergen.
- `server.py` sigue siendo un semi-god-module (858 líneas, fan-out 31) pese al split a
  `server_engine`/`server_core`; `server_core._JOBS` es un dict global in-memory
  compartido por referencia entre módulos (`server_core.py:14-16`).
- `__init__.py` importa 48 módulos hermanos eagerly: `import mmorch` paga el costo (y el
  riesgo de fallo) de TODO el paquete, incluido `evolve`, `code_loop`, `shadow_prior`.
- 46 `except Exception:`+`pass` en el paquete (medido); muchos son side-channels
  deliberados y comentados (ethos del CLAUDE.md), pero el volumen convierte la
  convención en cultura de fallo silencioso (existe `logs/silent_errors.jsonl` pero no
  todos pasan por ahí, ej. `mcp_server.py:399-400`).
- Estado global de proceso: `providers._CLIENTS` (`providers.py:32`),
  `memory._embedder`/`_MIGRATED_PATHS` (`memory.py:44,102`), `events` ring buffer,
  `budget._SPEND_CACHE` (`budget.py:26-31`) — todos singletons a nivel módulo.

---

## 7. Los 10 problemas arquitectónicos más graves para production-readiness

1. **ROOT hardcodeado al directorio del paquete (27 módulos).** Todo el estado vive en
   `Path(__file__).parent.parent` (`memory.py:32`, `feedback.py:26-28`,
   `metrics.py:22`, `intuition.py:30`, …). Instalado como wheel en site-packages el
   sistema escribiría dentro de la instalación; imposible correr dos instancias o
   separar código de datos. No existe `MMORCH_DATA_DIR`. Es EL bloqueo de deploy.

2. **Estado persistente sin gobierno: 69 archivos heterogéneos en `logs/` (58MB) + DBs
   en la raíz.** Estado aprendido crítico (bandits, adjudications), datasets de 11MB,
   caches, logs de proceso y backups `.bak` a mano conviven sin manifiesto, sin
   versionado de esquema, sin rotación ni política de backup/restore. `chat.db` y
   `workflow.db` están fuera de `logs/`. Una migración o un restore parcial hoy es
   arqueología.

3. **Concurrencia multi-proceso coordinada solo por convención.** MCP server, HTTP
   server, `nightly.py` y hooks escriben los mismos archivos desde procesos distintos.
   Mitigado ad-hoc: write atómico del bandit (`feedback.py:125-127`), lock de thread en
   metrics (`metrics.py:19,59`) — pero el lock es **por proceso**, no inter-proceso;
   appends JSONL concurrentes dependen de la atomicidad de `write()` del OS, y
   `memory.duckdb` es single-writer (DuckDB lockea el archivo: el segundo proceso que
   escriba memoria falla). No hay una capa de acceso a estado; cada módulo abre sus
   archivos.

4. **El presupuesto (única defensa de gasto) se computa re-derivando un JSONL sin
   rotación.** `budget.check()` corre antes de CADA call y suma el mes desde
   `metrics.jsonl` (5.4MB/18.5k líneas y creciendo) — cacheado por mtime
   (`budget.py:26-31`) pero acoplado a que el log jamás rote (el propio comentario de
   `metrics.py:66-68` lo declara invariante). Además el piso es reconocidamente
   incompleto: "calls timeouteadas loggean cost=0 pero el server factura"
   (`budget.py:9-10`). Control financiero sobre una fuente que se degrada con el uso y
   subestima por diseño.

5. **Doble bandit con caminos de actualización divergentes según entry point.**
   `record_outcome()` de librería entrena SOLO el sig-bandit (si hay context,
   `feedback.py:56-61`); el tool MCP `mmorch_record_outcome` además actualiza el bandit
   plano `bandit_state.json` (`mcp_server.py:585-587`). Mismo evento, aprendizaje
   distinto según por dónde entró. `bandit_state.json` lleva sin tocarse desde jun-30
   (168 bytes) mientras `bandit_sig.json` está vivo — un learner zombie que
   `mmorch_feedback_stats` sigue reportando como "el bandit".

6. **Servidor HTTP: auth opcional y jobs no durables.** Sin `MMORCH_SERVER_TOKEN` el
   server acepta todo (`server_core.py:20-25`, "modo dev"); el token viaja también por
   query string `?token=` (`server.py:11-12` lo documenta) → queda en logs de
   proxies/history. Los jobs corren en threads con registro en `_JOBS` dict in-memory
   (`server_core.py:14`): crash del proceso = jobs perdidos (hay checkpoints en
   `workflow_store` para rubric, pero el registro/kanban no sobrevive). "Control total
   remoto" descansa enteramente en la disciplina Tailscale del operador.

7. **`import mmorch` = importar los 48 módulos.** `__init__.py:13-60+` re-exporta
   eagerly desde ~48 hermanos. Un `SyntaxError` o una dep opcional rota en CUALQUIER
   módulo (evolve, shadow_prior, code_loop…) tumba el paquete entero, incluido el MCP
   server que solo necesita 15. También infla el arranque del server stdio. Falta un
   core mínimo importable + lazy para el resto.

8. **La capa MCP duplica lógica en vez de envolverla.** Comportamiento real vive en los
   wrappers: quarantine-handling, strip de code-fences (`mcp_server.py:630-635`),
   bridge memoria+babel-thread de vault_write (`mcp_server.py:374-401`), la doble
   actualización de bandit (#5), el regex de secretos (`mcp_server.py:184-186`). Un
   caller de librería (Lotus, workflows, tests) obtiene semántica DISTINTA que un
   caller MCP para "la misma" operación. Los 44 wrappers son además serialización a
   mano — cada campo nuevo se agrega en dos lugares o se pierde en silencio.

9. **Fallo silencioso institucionalizado.** 46 `except Exception`/`pass` en el paquete;
   los peores en rutas de aprendizaje/persistencia: si `intuition.record` falla, el
   outcome se registra pero nadie aprende y nadie se entera (`feedback.py:56-61`); si
   el babel-ingest muere, "el nightly barre" (`mcp_server.py:391-400`) — supuesto no
   verificado por nada. Para producción hace falta que TODO side-channel pase por un
   canal observable único (`silent_errors.jsonl` existe pero es opt-in por sitio).

10. **Precios y modelos hardcodeados-volátiles como fuente de verdad de costo.**
    `config.py` declara los precios "VOLATILE — re-verify" (`config.py:3-4`) y varios
    "PROVISORIOS copiados de 4.6" (`config.py:142-145`); todo el break-even, el budget
    (#4) y las recomendaciones de `learn` derivan de esos números. Existe el override
    (`prices.py`/megasource) pero el default sigue siendo constantes en código con
    fecha vencida: el costo reportado puede divergir de la factura real sin que ninguna
    alarma lo detecte (el propio `budget.py:9` lo admite). Un sistema cuya razón de ser
    es "ahorrar $" necesita conciliación contra facturación real, no autoreporte.

### Nota positiva final
La lógica de dominio (patrones, gates deterministas, cross-family enforced, memoria con
degrade graceful) es sólida y está inusualmente bien comentada con el *porqué*. La deuda
es casi toda **operacional**: dónde vive el estado, quién lo escribe concurrentemente,
qué pasa cuando algo falla en silencio, y cómo se instala fuera de `~/.claude`. Es la
deuda esperable de un sistema que creció por grafts en su directorio de origen — y es
exactamente lo que separa "anda en mi máquina siempre-prendida" de "production-ready".
