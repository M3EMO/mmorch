# 03 — Auditoria de red de seguridad: gates, tests, health checks

Fecha: 2026-08-27 · Repo: `C:/Users/map12/.claude/orchestration` (paquete `mmorch`, 129 archivos fuente, 22.764 LOC en `mmorch/`)
Entorno de gates: `.venv/Scripts/python.exe` (ruff 0.15.20, mypy 2.1.0). Nota: el Python de sistema (3.12) NO tiene ruff/mypy — los gates solo corren via el venv del repo.

---

## 1. Estado de los gates

| Gate | Comando | Estado | Detalle |
|---|---|---|---|
| ruff (lint) | `.venv/Scripts/python.exe -m ruff check .` | **VERDE** | "All checks passed" — ruleset F/E9/B/PLE (`pyproject.toml` `[tool.ruff.lint]`) |
| mypy (types) | `.venv/Scripts/python.exe -m mypy mmorch --ignore-missing-imports` | **ROTO** | **10 errores en 3 archivos** (el contrato en `pyproject.toml` [tool.mypy] dice "keep it at 0") |
| pytest | `pytest tests/ -q` | no corrido aqui (corre en otro proceso) | **718 tests colectados** en 3.76s, coleccion limpia |
| ste-lint (prosa) | `tools/ste-lint.py` | existe, manual | gate deterministico para markdown generado; sin wiring a hook/nightly del repo propio |
| CI | — | **NO EXISTE** | No hay `.github/workflows/` ni ningun CI. Todo el enforcement es local |

### 1.1 mypy: los 10 errores (regresion del gate)

- `mmorch/health.py:162` — `error: Assignment to variable "e" outside except: block [misc]` — `e = _json.loads(ln)` reusa el nombre `e` ligado por un `except` previo. Check nuevo de mypy 2.x.
- `mmorch/provenance.py:38` — `error: Module has no attribute "flock" [attr-defined]` — `return fcntl.flock` dentro del `try/ImportError`; mypy en win32 sabe que `fcntl` no tiene `flock`. Falso-positivo de plataforma en codigo correcto en runtime, pero rompe el 0.
- `mmorch/regresion.py:231,233,235,238` — 8 errores `arg-type`: `kw = dict(repo="r", ...)` se infiere `dict[str, object]` y se desempaca `**kw` en `refutar_ejecutable(...)` (self-check `__main__`). Introducido por los commits recientes de refutacion ejecutable (`3700715`, `90f18e9`).

**Causa raiz de la deriva (2 factores):**
1. **El hook pre-commit ACTIVO no corre mypy.** `git config core.hooksPath` apunta a `.beads/hooks/`; `.beads/hooks/pre-commit` corre SOLO `ruff check mmorch mcp_server.py tests` + docgen. El `.pre-commit-config.yaml` de la raiz (que SI define el hook mypy local, "mypy is at 0 — keep it there") corresponde al framework `pre-commit`, que escribe en `.git/hooks/` — **bypaseado por `core.hooksPath`**. El gate mypy hoy no lo ejecuta nadie automaticamente.
2. **Drift de version:** venv tiene mypy 2.1.0; `pyproject.toml` pide `mypy>=1.10`. mypy 2.x agrega checks (p.ej. el `[misc]` de health.py) — el "0" se calibro con otra version y no hay pin.

### 1.2 Alcance parcial de los gates

- mypy solo cubre `mmorch/` — `mcp_server.py` (el entrypoint MCP de 46 tools), `scripts/` (nightly, smoke, gate_hardening) y `tools/` quedan fuera del type gate.
- ruff-hook cubre `mmorch mcp_server.py tests` pero no `scripts/` ni `tools/` (aunque `ruff check .` manual si pasa hoy en todo el repo).

---

## 2. Inventario de tests

- `tests/`: **99 archivos test_*.py** (+`mut_signature.py` helper), **718 tests colectados**.
- `mmorch/`: 128 modulos + `__init__` — **91 modulos referenciados por algun test**, **37 sin ninguna referencia en tests/**.

### 2.1 Modulos SIN tests, CON self-check `__main__` (30)

`python -m mmorch.<mod>` corre asserts inline (convencion del repo; ruff per-file-ignore B011 la protege), pero **nadie los ejecuta en bulk** — ni pytest, ni smoke.py, ni nightly:

`arbitration, babel, bench, bursts, chat_store, code_review, context_blocks, debate, durable_runs, evolve_findings, exec_policy, feedback_trace, gate_policy, job_graph, lang, mcp_telemetry, minds, plugin_worker, plugins, portability, project_build, project_driver, project_integrate, speedup, textutil, workflow_engine, workflow_evolve, workflow_race, workflow_spec, workflow_store`

Riesgo: los self-checks son verdad de ejecucion SOLO si alguien los corre; hoy son documentacion ejecutable latente. Un runner que itere `python -m mmorch.X` sobre esta lista convertiria 30 modulos de cobertura-cero a cobertura-smoke con un script de ~15 lineas.

### 2.2 Modulos SIN tests y SIN self-check (7) — cobertura CERO real

| Modulo | Nota |
|---|---|
| `mmorch/pty_session.py` | PTY/ConPTY — dificil de testear, pero cero red |
| `mmorch/server_core.py` | descompuesto de server.py; solo cubierto INDIRECTO via `tests/test_server_smoke.py` (contrato de rutas de server.py) |
| `mmorch/server_engine.py` | idem indirecto |
| `mmorch/server_fleet.py` | idem indirecto (test_fleet.py testea `fleet.py`, no `server_fleet`) |
| `mmorch/server_frontend.py` | idem indirecto |
| `mmorch/server_pty.py` | ni indirecto ni self-check |
| `mmorch/transcript_store.py` | store de transcripts sin ninguna verificacion |

`tests/test_server_smoke.py` es un buen contrato (tabla de rutas exacta + auth 401 + home sin auth, sin side-effects) pero solo prueba lo que server.py registra, no la logica interna de los `server_*`.

### 2.3 mcp_server.py (46 tools)

No hay un test que enumere las 46 tools MCP y valide el contrato nombre→handler→schema (el analogo de test_server_smoke pero para MCP). `mcp_server.py` ademas esta fuera del gate mypy.

---

## 3. Health checks existentes

### 3.1 `mmorch/health.py` — dead-man's switch

- `beat(component)` → `logs/health.jsonl`; `check()` clasifica dead/alive/never contra `EXPECTATIONS = {"nightly": 26h, "server": 900s, "digest": 26h}` (`mmorch/health.py:10`).
- `report()` combina check + scrape_errors (server_forever.err, nightly.jsonl) + silent_errors.jsonl 48h → flag `healthy`.
- `check_projects()` corre el pytest de cada proyecto de `projects.json` con SU propio venv (orchestration se auto-incluye — el propio repo SI corre su suite cada noche por esta via).

**Estado actual (medido hoy): `healthy=False`**
- `nightly` **DEAD** — overdue ~15.7h sobre el limite de 26h (ultimo beat hace ~41.7h).
- `server` y `digest` **NEVER** — y esto es estructural: **el unico caller de `beat()` en todo el repo es `scripts/nightly.py:289` (`beat("nightly")`)**. Ni `mmorch/server.py` ni el pipeline de digest emiten heartbeat nunca. `EXPECTATIONS` declara 3 componentes, 1 solo tiene emisor → `server`/`digest` estaran en `never` para siempre y el `healthy=False` cronico entrena a ignorar la alarma.
- `server_err_tail`: loop de `[Errno 10048] bind 127.0.0.1:8787` — `scripts/server_forever.ps1` relanza contra un puerto ya ocupado (doble instancia o zombie), error repetido en el tail.

### 3.2 `scripts/smoke.py` — smoke de subsistemas (read-only, cero LLM)

Corrido hoy: **13/13 OK** (fuel, curation, adjudicate, health, outcomes, automerge, merge_train, decision_mining, flywheel, descubrimiento, reflexion, budget, server /pending). Nightly lo corre y persiste `logs/smoke.jsonl`; `scripts/manana.py` lo reporta.

Trampa detectada: el check "health (report)" da ✓ con detalle `healthy=False, dead=1, never=2` — **pasa porque report() ejecuto, no porque el sistema este sano**. El smoke verde convive con el sistema declarandose no-healthy.

### 3.3 `smoke_test.py` (raiz) — smoke E2E cross-family

fan_out DeepSeek + adversarial_verify Gemini con bug plantado. Requiere API keys (.env), gasta USD real → correcto que sea manual, pero no esta en ninguna rutina.

### 3.4 `scripts/gate_hardening.py`

Gate por verdad de ejecucion para el hardening loop (mutantes muertos + suite verde). Bien diseñado; scope = worktrees del loop, no health general.

---

## 4. CI / automatizacion

- **No hay CI** (no `.github/`, no runner externo). Enforcement = hook pre-commit local (solo ruff) + nightly (Task Scheduler `mmorch-nightly` 02:10 → smoke.py + check_projects + evolve) + `mmorch-autopull` cada 15 min.
- El propio nightly esta muerto ahora mismo (3.1), asi que hoy la unica red activa es el hook de ruff.
- Bypass trivial documentado en el propio hook: `git commit --no-verify`.

---

## 5. Que falta para un health-check/smoke de sistema completo autoejecutable

Prioridad descendente:

1. **Reparar el gate mypy** (10→0): fix `regresion.py` (tipar `kw` o pasar args explicitos), renombrar `e` en `health.py:162`, `# type: ignore[attr-defined]` o `sys.platform`-guard en `provenance.py:38`. Pin de mypy (`mypy>=2.1,<3`) para que "0" signifique siempre lo mismo.
2. **Mover el gate mypy al hook ACTIVO**: agregar la linea mypy a `.beads/hooks/pre-commit` (o repuntar `core.hooksPath`); hoy `.pre-commit-config.yaml` es letra muerta.
3. **Emitir los heartbeats declarados**: `beat("server")` en el loop del server (cada <900s) y `beat("digest")` al refrescar digest en nightly — o recortar `EXPECTATIONS` a lo que realmente late. Objetivo: que `healthy=True` sea alcanzable y `False` sea señal, no ruido.
4. **Runner de self-checks**: script (o test parametrizado `test_selfchecks.py`) que corra `python -m mmorch.<mod>` para los 30 modulos de 2.1, subprocess con timeout, y lo sume a smoke.jsonl. Convierte 30 modulos sin tests en cubiertos-por-smoke sin escribir un test nuevo.
5. **Contrato MCP**: test tipo test_server_smoke para `mcp_server.py` — enumerar las 46 tools, validar schema/handler, y congelar la tabla (cachar tool dropeada en refactors). Incluir `mcp_server.py` en el gate mypy.
6. **Tests directos para los 7 de cobertura cero** (2.2), empezando por `transcript_store` y `server_pty` (los unicos sin NINGUNA verificacion, ni indirecta).
7. **Un solo comando `system-check`**: encadenar ruff + mypy + `pytest -q` + `scripts/smoke.py` + `health.report()` (exit≠0 si healthy=False) + contrato server/MCP. Hoy existen todas las piezas pero no hay un entrypoint que de un veredicto unico; nightly ya orquesta parte, pero no corre ruff/mypy y esta muerto cuando mas se lo necesita.
8. **Arreglar server_forever.ps1**: chequear puerto 8787 libre (o matar instancia previa) antes de relanzar — el tail de errores 10048 en loop contamina scrape_errors.
9. **(Opcional) CI minimo**: GitHub Actions con ruff+mypy+pytest en push — el repo ya pushea a github.com/M3EMO/mmorch; seria el unico enforcement no-bypasseable con `--no-verify`.

---

## Apendice: comandos de gate canonicos (Windows, este repo)

```
.venv/Scripts/python.exe -m ruff check .
.venv/Scripts/python.exe -m mypy mmorch --ignore-missing-imports
.venv/Scripts/python.exe -m pytest tests/ -q
.venv/Scripts/python.exe scripts/smoke.py
.venv/Scripts/python.exe -c "from mmorch.health import report; import json; print(json.dumps(report(logs_dir='logs')))"
```
