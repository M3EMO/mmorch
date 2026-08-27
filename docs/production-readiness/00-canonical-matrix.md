# 00 — Matriz canónica de producto (consolidado Research)

Fecha: 2026-08-27 · Consolidado de 01-architecture-map, 02-interface-contracts,
03-quality-gates-tests, 04-self-evolution-state, 05-known-defects-backlog,
06-cursor-standalone-research. HEAD `90f18e9`. Evidencia en `file:line`.

**Leyenda de estados** (brutal por diseño):
- **sólido** — corre en producción real, con tests o logs de ejecución que lo prueban.
- **frágil** — corre, pero con defectos conocidos, degradación silenciosa o cobertura parcial.
- **aspiracional** — el plan/GOAL lo declara; el código existe parcialmente o no está en el camino vivo.
- **decorativo** — implementado y hasta testeado, pero NADIE lo ejecuta en el camino real (museo).

---

## (a) Features canónicas: qué ES mmorch

### 1. Orquestación multi-modelo (núcleo del producto)

| Capacidad | Estado | Evidencia |
|---|---|---|
| `providers.call()` — único punto de salida a APIs, budget-gate + telemetría + error-class + client cache detrás de una firma | **sólido** | providers.py:104-205; budget gate :129-139; `_classify_error` :35 |
| Patrones: fan_out, adversarial_verify, route, cascade, ensemble, tournament, bucket_rank, classify, cynefin, innovate | **sólido** | módulo por patrón, dataclass de resultado; convergen en providers.call (01 §1) |
| Cross-family enforced (generador↔verificador de familias distintas; par same-family ⇒ raise) | **sólido** | config.family_of (config.py:169); mcp_server.py:87-121 |
| ... pero el guard confía en el `gen_model` DECLARADO por el caller — mislabel = bypass | **frágil** | patterns.py:152-158 (H-4, ABIERTO desde jun-06) |
| Retry/backoff ante 429/5xx del provider | **aspiracional** | NO EXISTE — providers.py clasifica errores pero no reintenta (H-7); único retry es JSON-parse (schema.py:91-129) |
| `bucket_rank`: excepción en `f.result()` aborta TODO el pool y pierde lo ya procesado | **frágil** | bucketrank.py loop `as_completed` (05 #5, mismo patrón que H-1 ya fixeado en fan_out) |
| Spec/prompt-sharpening: spec_interview, build_spec (cuarentena de scope), perfect | **sólido** | mcp_server.py:500-528, 829; cuarentena documentada (02 §1.2) |
| Schema gates: gated_json, validate, extract_json | **sólido** | exportados en `__init__` (02 §2) |
| 21 checkers deterministas (arithmetic…unit_test…sympy) | **sólido** | checkers.py:570-592; pero docstring MCP declara solo 2 (mcp_server.py:609) |

### 2. Aprendizaje / capa cognitiva

| Capacidad | Estado | Evidencia |
|---|---|---|
| Sig-bandit de intuición (Thompson por firma estructural) | **sólido** | intuition.py:34-37; bandit_sig.json vivo; entrenado desde feedback.py:56-61 |
| Bandit plano `bandit_state.json` | **decorativo** (zombie) | sin tocar desde jun-30 (168B); solo lo actualiza la vía MCP (mcp_server.py:585-587); `mmorch_feedback_stats` lo reporta como "el bandit" |
| Doble camino de aprendizaje divergente (librería entrena solo sig-bandit; MCP entrena ambos) | **frágil** | mcp_server.py:585-587 vs feedback.py:56-61 — mismo evento, aprendizaje distinto según entry point |
| Bandit sin recency decay (shift de paradigma → priors viejos misrutean) | **frágil** | feedback.ThompsonBandit (05 #10; decay 0.995 existe pero sin discounting por recencia real) |
| Memoria DuckDB 2 capas (episódico inmutable + semántico destilado + embeddings 384d, degrade graceful sin fastembed) | **sólido** | memory.py:105-153, :17-24; DDL con migración por columna |
| Recall (semántico + fallback episódico + RRF híbrido) — OJO: NO read-only, muta access_count | **sólido** | memory.py:391-403; contrato implícito no documentado (02 §5.9) |
| Retención/olvido Ebbinghaus, consolidate, tensiones, open loops | **sólido** | memory.py; consolidate dry-run por default (mcp_server.py:689) |
| Notas semánticas verificadas UNA vez, nunca re-verificadas (staleness) | **frágil** | 05 #13 |
| Ingest de sesiones + playbooks (flywheel) | **frágil** | sessions.py:183 — depende del formato JSONL de `~/.claude/projects` (Claude Code only; ciego en Cursor/standalone) |
| Vault global (write_validated + MOC + babel bridge) | **sólido** | vault.py:117-147; babel-ingest en thread daemon best-effort (mcp_server.py:391-401) |
| Outcome horizon post-merge (aprender de lo que falla DESPUÉS del merge) | **aspiracional** | NO EXISTE (05 #3, TOP del blind-spot triage); reap_merged_prs cablea el veredicto humano pero señal = 4 PRs |
| Shadow prior F5 | **decorativo** (dormido por dato, decisión medida) | scale=0 (04 §4) |

### 3. Ejecutores / motor de proyecto

| Capacidad | Estado | Evidencia |
|---|---|---|
| project_build engine F1-F4 (decompose recursivo + coder loop + verificador frío + gate de integración) | **sólido** | project_build.py:1-40 (validador de plan determinista, allowlist test_cmd anti-RCE); memoria: 7 bugs fixeados en live run |
| Ejecutor `run_claude` (claude -p headless) | **frágil** (acople) | claude_exec.py:26-33 — único backend; sin Claude Code CLI no hay coder loop; sin abstracción Executor |
| rubric_loop (executor/judge por state machine, checkers server-side) | **sólido** | rubric_loop.py:85-183; submit en fase terminal ⇒ ValueError |
| autoresearch / hillclimb (keep/discard sobre scorer congelado) | **sólido** con bug | autoresearch.py:44-55: `resume_from_journal` con loop muerto de `best` + rounds infladas post-crash (05 #16) |
| speedup (score = ejecución real en subprocess) | **sólido** | mcp_server.py:842 |
| Circuit-breaker de USD por-run dentro de builds recursivos | **aspiracional** | NO EXISTE (05 #2) — solo el cap mensual global |
| Sandbox de OS para test_cmd (worktree = aislamiento git, no de permisos) | **aspiracional** | 05 #1 — el test_cmd propuesto por LLM corre con permisos completos del usuario |

### 4. Auto-evolución (el claim central del GOAL)

| Capacidad | Estado | Evidencia |
|---|---|---|
| Loop nocturno harvest→propose→sandbox→PR (Task Scheduler, sin Claude, sin cupo) | **sólido** | nightly.py + evolve.py:557-697; 35 corridas en nightly.jsonl; 4 PRs (todos 08-25 tras fix fence/basetemp) |
| Zona roja en el camino VIVO (paths + delta-scan de contenido, reusado por automerge) | **sólido** | evolve.py:270-311, :580; automerge.py:37-63 |
| Tamper-halt GOAL.hash | **frágil** | goal.py:46-60 existe y verifica, PERO solo gatea `evaluate()`/`self_evolve()` — caminos que el nightly NUNCA ejecuta (04 §3.1); y borrar GOAL.hash re-autoriza solo (goal.py:53-54) |
| fitness() de 6 checks (goal_aligned + ensemble + cost + rollback + tamper) | **decorativo** | evaluate() evolve.py:88-143: implementada, testeada, CERO callers en el camino vivo (04 §3.4) |
| self_evolve(do_apply), promote_branch, pursue_goal, archive_variant, rollback() estructural | **decorativo** | sin caller fuera de tests (04 §4); evolution_archive.jsonl no existe |
| Automerge carril verde (único auto-apply real) | **aspiracional** (latente) | automerge.py:66 cableado desde auto_repair/hardening pero su ledger obligatorio NO existe ⇒ 0 ejecuciones jamás; + bug de falsos rojos en archivos nuevos con "password"/"secret" (05 #7) |
| auto_repair, hardening, merge_train, triage mecánico | **sólido** con bugs | logs de ejecución reales; auto_repair persiste estado ANTES del automerge (05 #6) + path Windows hardcodeado (auto_repair.py:91) |
| Kill-switch `logs/loop_paused` | **sólido** | chequeado en 4 módulos consistentemente (04 §3.6) |
| Auto-aplicación real de cambios | **0%** — hoy es pipeline con merge humano | 3 trenes verdes esperan click desde 08-22 (04 §2) |

### 5. Infra / operación

| Capacidad | Estado | Evidencia |
|---|---|---|
| Budget mensual (MMORCH_MAX_MONTHLY_USD) | **frágil** | única defensa de gasto; re-deriva metrics.jsonl sin rotación; subestima por diseño (timeouts loggean cost=0, budget.py:9-10); sin cap ⇒ ilimitado |
| Precios (fuente del costo/break-even) | **frágil** | hardcodeados "VOLATILE jun-2026" (config.py:3-4,142-145); sin `price_asof`; override megasource existe pero el default silencioso es el registry |
| Telemetría (metrics.jsonl + mcp_calls.jsonl por tool) | **sólido** | providers.py:161-196; mcp_telemetry.py:24-47 — cumple práctica 2026 |
| `metrics.summary()` revienta con una línea incompleta (sin .get()) | **frágil** | metrics.py:150-164 |
| Health check (dead-man's switch) | **frágil** | health.py: `healthy=False` CRÓNICO — de 3 componentes declarados solo `nightly` emite beat (único caller: nightly.py:289); server/digest = NEVER para siempre ⇒ la alarma entrena a ignorarse |
| Server HTTP (starlette, ~30 rutas, SSE, Lotus cliente) | **frágil** | token OPCIONAL (vacío = sin auth, server_core.py:20-25); token por query string; jobs en dict in-memory (crash = pérdida); sin `GET /health`; server_forever.ps1 en loop de bind 10048 |
| Gates estáticos ruff+mypy "en 0" | **ROTO hoy** | ruff verde; mypy 10 errores en 3 archivos (regresion.py, health.py:162, provenance.py:38); el hook activo (.beads/hooks) corre SOLO ruff — el hook mypy es letra muerta; sin pin de versión |
| Suite de tests (718 colectados, 99 archivos) | **sólido** con huecos | 37/128 módulos sin referencia en tests; 30 con self-check `__main__` que NADIE corre; 7 con cobertura CERO real (server_*, pty_session, transcript_store); mcp_server.py (46 tools) sin test de contrato ni mypy |
| CI | **NO EXISTE** | sin .github/; enforcement = hook local (solo ruff) bypasseable con --no-verify |
| MCP server (46 tools stdio FastMCP) | **sólido** con contrato sucio | contrato de error inconsistente ({"error"} vs excepción cruda), docstrings desactualizados, validación de bordes con 9 huecos concretos (02 §4) |
| Instalable como paquete (pipx/uv) | **aspiracional** | 25-27 módulos anclan estado a `Path(__file__).parents[1]`; mcp_server.py/prompts/roles fuera del paquete — solo funciona como checkout editable |
| Compatible Cursor | **aspiracional** (cerca) | stdio compatible casi tal cual, PERO: 46 tools > techo práctico 40; `.env` resuelto por cwd (providers.py:19) no carga con cwd de workspace; hooks/skills/ingest no viajan |
| Concurrencia multi-proceso (MCP + server + nightly sobre los mismos archivos) | **frágil** | locks solo por-proceso; DuckDB single-writer; write atómico solo en bandit; coordinación por convención |
| Fallo silencioso: 46 `except Exception/pass` | **frágil** (cultural) | incluso en rutas de aprendizaje (feedback.py:56-61); silent_errors.jsonl es opt-in por sitio |

---

## (b) Arquitectura objetivo (1 página)

### La actual (funciona, no viaja)

```
Claude Code ──stdio──> mcp_server.py (46 tools, wrappers con lógica propia)
Lotus/HTTP ──token?──> mmorch.server (jobs in-memory)
Task Scheduler ──────> scripts/nightly.py (evolve/repair/hardening/train → PR humano)
                └──── todos escriben logs/ (69 archivos, 58MB) + DBs en la raíz,
                      rutas ancladas a Path(__file__).parents[1] en ~27 módulos
```

### La objetivo (mínimo delta para production-ready + standalone + Cursor)

```
[cliente MCP: Claude Code | Cursor]        [Lotus / curl]        [scheduler OS]
        │ stdio (perfil core ≤40 tools          │ HTTP token          │
        │  o streamable-http compartido)        │ OBLIGATORIO         │
        ▼                                       ▼                     ▼
  mmorch.mcp_server (DENTRO del paquete,   mmorch.server        mmorch nightly
  entry point `mmorch-mcp`, wrappers        + GET /health        (goal_guard al
  SIN lógica: todo baja a la librería)                            arranque)
        └──────────────┬────────────────────────┴─────────────────────┘
                 librería mmorch (única semántica; Executor abstraído:
                 backend claude-CLI | cursor-agent | API-only)
                        │
                 MMORCH_HOME (~/.mmorch): TODO el estado — logs/, DBs,
                 bandits, memoria — con manifiesto, rotación de metrics.jsonl
                 y backup/restore; código instalable por pipx/uv aparte
```

**Los 8 cambios mínimos, en orden de dependencia:**
1. **`MMORCH_HOME` + `data_dir()`** en config.py reemplaza los ~27 `parents[1]` — EL habilitante (deploy, dos instancias, backup). Sin esto nada de lo demás importa.
2. **mcp_server dentro del paquete** + entry point `mmorch-mcp`; `.env` resuelto por `__file__`, no cwd → registrable en Cursor y por `uvx`.
3. **Perfil `MMORCH_MCP_PROFILE=core`** (≤40 tools) para el techo de Cursor.
4. **mypy 10→0 + pin + mover el gate al hook ACTIVO** (.beads/hooks) + CI mínimo (ruff+mypy+pytest en push) — hoy el "0" es ficción.
5. **Health honesto**: emitir `beat("server")`/`beat("digest")` (o recortar EXPECTATIONS) + `GET /health` + fix server_forever.ps1 → que `healthy=True` sea alcanzable y `False` sea señal.
6. **Token del server OBLIGATORIO** (arranque sin token = refuse, no "modo dev") + token solo por header.
7. **Unificar el gate de evolución**: goal_guard al arranque del nightly (1 línea) + goal_aligned sobre el diff pre-PR, o sincerar GOAL.md; primer automerge verde con ledger verificado.
8. **retry/backoff en providers.py** + fix pool bucketrank — cierran el fallo duro en runs masivos.

**Lo que NO cambia**: la capa de patrones, la memoria, los checkers, el diseño cross-family, el pipeline nocturno PR-based (es más seguro que el plan original). La deuda es operacional, no de dominio.

---

## (c) Gaps rankeados por impacto

| # | Gap | Impacto | Por qué este rank | Fuente |
|---|---|---|---|---|
| 1 | **ROOT anclado a la instalación (27 módulos), sin MMORCH_HOME** | BLOQUEA deploy, packaging, Cursor-por-uvx, dos instancias, backup | Es el prerequisito de casi todo lo demás | 01 #1, 06 B.2.1 |
| 2 | **Gates estáticos rotos y sin enforcement**: mypy 10 errores, hook activo solo corre ruff, sin CI | El contrato "en 0" es hoy mentira; toda regresión futura entra sin fricción | La red de seguridad barata que ya se pagó no está conectada | 03 §1 |
| 3 | **Health check crónicamente rojo + smoke que pasa con healthy=False** | Las alarmas entrenan a ignorarse; el nightly estuvo MUERTO ~41h sin que nada gritara | Un sistema desatendido con alarma rota no es desatendible | 03 §3 |
| 4 | **Server HTTP sin auth obligatoria + jobs no durables** | "Control total remoto" descansa en disciplina Tailscale; crash = jobs perdidos | Único gap con cara de incidente de seguridad | 01 #6 |
| 5 | **El relato de seguridad de la auto-evolución ≠ el código que corre**: fitness()/goal_aligned/tamper-halt decorativos; auto-apply 0% con ledger inexistente | Integridad del claim central del producto; imposible razonar sobre autonomía futura | No es riesgo hoy (todo pasa por humano) pero invalida el GOAL como documento | 04 §3-5 |
| 6 | **Budget frágil como única defensa de gasto**: subestima por diseño, JSONL sin rotación, precios vencidos sin conciliación, sin breaker por-run | Un run patológico o un precio desactualizado quema plata sin corte ni alarma | La razón de ser del producto es "ahorrar $" | 01 #4/#10, 05 #2 |
| 7 | **Semántica librería ≠ semántica MCP** (lógica en wrappers, doble bandit, contrato de error inconsistente) | Lotus/workflows/Cursor obtienen comportamiento distinto por "la misma" operación; el bandit aprende distinto según puerta | Deuda que se multiplica con cada cliente nuevo | 01 #5/#8, 02 §5 |
| 8 | **Sin retry/backoff + pool de bucketrank abortable** | Runs masivos nocturnos fallan duro por un 429 transitorio | Fix chico, patrón ya conocido (H-1) | 05 #4/#5 |
| 9 | **37 módulos sin tests (7 con cero real), 30 self-checks que nadie corre, mcp_server sin test de contrato** | Cobertura ilusoria justo en los entry points | Un runner de ~15 líneas convierte 30 módulos a smoke | 03 §2 |
| 10 | **Concurrencia multi-proceso por convención + estado sin gobierno (69 archivos, sin schema/rotación/backup)** | Corrupción silenciosa posible; migración/restore = arqueología | Real pero de baja frecuencia local-scale; sube con streamable-http multi-cliente | 01 #2/#3 |
