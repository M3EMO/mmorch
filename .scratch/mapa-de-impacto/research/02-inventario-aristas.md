# Inventario de aristas invisibles en mmorch (ticket 02)

Fecha: 2026-09-24. Alcance: `mmorch/*.py` (124 módulos) y `scripts/*.py`. Tests excluidos del conteo.
Método: grep, AST de stdlib y consultas a codegraph v0.9.6. No se corrió la suite.
Índice codegraph al medir: 341 archivos, 5717 nodos, 11169 aristas.

## Tabla resumen

| Clase | Conteo | ¿codegraph la ve? | Ejemplo |
|---|---|---|---|
| 1a. Datos: archivos json/jsonl/db | ~89 almacenes reales (104 nombres, ~15 son fixtures de autotest); 19 tocados desde más de un archivo; ~75-80 pares escritor→lector (aprox.) | No | `nightly.py:55 _log` escribe `nightly.jsonl`; lo leen 6 funciones en 5 módulos |
| 1b. Datos: centinela `loop_paused` | 1 escritor, 11 lectores en 11 módulos | No | `auto_apply.py:200` escribe; `automerge.py:93` y otros 10 leen |
| 1c. Datos: SQL directo sobre DB ajena | 2 accesos que saltean al dueño (memory.duckdb); workflow.db tiene dueño único | No | `curiosity.py:41-44` hace `SELECT ... FROM semantic` |
| 1d. Datos: variables de entorno | 43 variables reales (37 `MMORCH_*`); solo 5 se escriben en código | No | `budget.py:37` lee `MMORCH_MAX_MONTHLY_USD`; nada en el repo la define |
| 2a. Indirectas: función como valor | ~104 sitios (aprox.) | No | `auto_apply_nightly.py:163 signal_fn=collect_signals` |
| 2b. Indirectas: decoradores de registro | 67 aplicaciones (44 `@_tool`, 14 `@check`, 9 `@stage`) | Parcial: `impact` sí, `callers`/`trace` no | `mcp_server.py:278 @_tool` |
| 2c. Indirectas: despacho por string | 12 sitios (9 `dict[k]()`, 1 `getattr`, 1 `importlib`, 1 RPC) | No | `checkers.py:609 _REGISTRY[name](**ctx)` |
| 2d. Indirectas: import con alias | 84 `from X import f as g` | No | `canary.py:23,114 record_outcome as _record_outcome` |
| 3. Ejecución | ~26 aristas concretas: 12 subprocesos, 5 hooks, 3 tareas programadas, 4 entrypoints, 2 pre-commit | No | `auto_apply_nightly.py:50` lanza `scripts/gates.py` |
| 4. (Extra) Índice desactualizado | 4 módulos con 0 aristas entrantes de otros archivos: metrics, budget, learn, sdlc | Pierde aristas de código que antes veía | `codegraph_callers(log_event)` = 0; hay 8 llamadas en providers y patterns |

## Clase 1 — Aristas de datos

### Método (reproducible)

- Almacenes de archivo: recorrer el AST y tomar cada `ast.Constant` str. El nombre base debe cumplir `^[A-Za-z0-9_.\-]+\.(jsonl|json|db|duckdb|sqlite3?)$`. Agrupar por nombre base y contar archivos distintos.
- Centinelas: `grep -rn "loop_paused" mmorch scripts --include=*.py`.
- DB: `grep -rnE "sqlite3\.connect|duckdb\.connect|_connect\("` y `grep -rnE "FROM (semantic|episodic)"`.
- Entorno: llamadas AST a `os.getenv`/`os.environ.get` con clave constante, más `os.environ[...]`. Tres claves son falsos positivos (`constraints`, `files`, `tools`).

### Almacenes compartidos principales

| Almacén | Escritores | Lectores | ¿Dueño único? |
|---|---|---|---|
| `logs/metrics.jsonl` | `metrics.log_event` (metrics.py:76), llamado en providers.py:291,301,343,372,399 y patterns.py:81,157,194 | `budget.monthly_spend` (budget.py:61), `learn.analyze` (learn.py:21), `schedule` (schedule.py:56), `auto_apply_nightly.collect_signals` (:73), `sdlc` ledger (sdlc.py:695), `metrics.error_rates/cache_stats/summary` | Sí, desde 2f14d4f (`metrics.read_events`) |
| `logs/nightly.jsonl` | `nightly._log` (nightly.py:55) | `auto_repair.repair` (auto_repair.py:82), `hardening.load_last_map` (hardening.py:22), `health.scrape_errors` (health.py:115), `loop_nightly.reflect` (loop_nightly.py:211), `project_repair.repair_projects` (project_repair.py:44), `nightly.main` (nightly.py:584) | No. El contrato son claves string: `rec["bughunt"]["worst"]` se escribe en nightly.py:354 y se lee en hardening.py:31 |
| `logs/adjudications.json` | `adjudicate.run_incremental` (:167), `outcomes._save_adjudications` (:25), `proposals.compose_cards` (:44), `proposals.pick_card` (:74) | `curation.pending` (:39), `loop_nightly.run_idea_loop` (:594), `outcomes._load_adjudications` (:19), `proposals.compose_cards` (:32), `export_training_data` (:85), `smoke.c_adj` (:52) | No. outcomes.py tiene load/save, pero 3 módulos escriben directo |
| `loop_paused` (centinela) | `auto_apply._pause` (auto_apply.py:200) | adjudicate.py:11, automerge.py:93, auto_repair.py:77, hardening.py:70, loop_nightly.py:525, merge_train.py:46, project_repair.py:40, proposals.py:30, repo_mining.py:238, self_audit.py:185, slim.py:45 | No |
| `logs/feedback.jsonl` | `feedback.record_outcome` (dueño feedback.py:28) | loop_nightly.py:20 (`FUEL_PATHS`), export_training_data.py:76, smoke.py:76, hillclimb.py:31 (importa `_FEEDBACK_LOG`) | No: 3 lectores saltean al dueño |
| `logs/smoke.jsonl` | `scripts/smoke.py:187` | `nightly.main` (:511), `manana.seccion_salud` (:53) | No |
| `logs/silent_errors.jsonl` | `nightly.main` (:600) | `health._recent_silent_errors` (:152) | No |
| `logs/focus.json` | `loop_nightly.apply_focus` (:287) | `hardening.harden` (:80) | No |
| `logs/automerge_ledger.jsonl` | `automerge.try_automerge` (:141) | `auto_apply._hide_ledger` (:245) | No |
| `logs/merge_train.jsonl` | `merge_train.run_train` (:150) | `manana._tren_rojo_de_anoche` (:69) | No |
| `logs/workflow_obs.jsonl` | `session_skills` (`_STORE`, :22) | loop_nightly.py:20 (`FUEL_PATHS`) | No |
| `memory.duckdb` | `memory.py` (22 sentencias SQL) | `curiosity.py:41-44` y `nightly.py:234` con SQL crudo sobre `semantic` y `episodic` | No: 2 accesos saltean al dueño y dependen de columnas (`tombstone`, `needs_review`, `kind`) |
| `workflow.db` | `workflow_store.py:29` | 4 módulos vía workflow_store | Sí |
| `logs/context_blocks.db` | Nadie. El commit 19601ef (2026-09-14) borró el escritor | Nadie en el repo | Huérfano. Ver clase 3 |

### Hallazgos de riesgo

- **Kill-switch con ruta distinta.** `auto_apply._pause` escribe en `PromotionStore.root`. Ese root es `logs/auto_apply` (auto_apply_nightly.py:168). Los 11 lectores miran `logs/loop_paused`. Ningún lector mira `logs/auto_apply/loop_paused`. El test (test_auto_apply.py:218) solo verifica que el archivo exista. Conclusión provisoria: un halt de auto-apply no pausa los otros loops. Falta confirmar la intención con el dueño.
- **Dos raíces para el mismo archivo.** `scripts/smoke.py`, `manana.py` y `export_training_data.py` usan la raíz del repo (`parents[1] / "logs"`). `nightly.py` usa `_mmorch_home()`. `runtime_checkout.py:54` fija `MMORCH_HOME` a otra carpeta. Con rollout activo, escritor y lector de `smoke.jsonl` pueden apuntar a carpetas distintas. No lo verifiqué en ejecución.
- **Techo de gasto fuera del repo.** `budget.py:37` lee `MMORCH_MAX_MONTHLY_USD`. El `.env` del repo no la define; solo define `MMORCH_CODEGRAPH` y `MMORCH_SERVER_TOKEN`. Puede estar en el entorno del usuario. No lo verifiqué.

### Entorno

- 43 variables reales. Solo 3 se leen en más de un archivo: `PATH`, `MMORCH_SERVER_HOST` y `MMORCH_SERVER_TOKEN`.
- 5 se escriben en código: `MMORCH_HOME` (runtime_checkout.py:54, env del subproceso), `PYTHONPATH` (plugins.py:149), `PATH` (sdlc.py:1356), `MMORCH_ZOMBIE_TTL` (durable_runs.py:78), `MMORCH_BLOCK_GC` (workflow_store.py:335).
- Las otras ~38 se definen fuera del código: `.env`, Task Scheduler, shell o docs. Ejemplo: `MMORCH_AR_SCORER` (nightly.py:187) elige qué script ejecuta autoresearch. Es una arista de entorno y de ejecución a la vez.

### Prueba con codegraph

- `codegraph_impact(log_event, depth 1)` devolvió 2 símbolos: la función y su archivo. Ningún lector.
- `codegraph_callers(read_events)` devolvió 3 llamadores, todos dentro de metrics.py. Faltan budget, learn, schedule, sdlc y auto_apply_nightly. La causa es la clase 4, no solo la arista de datos.
- `codegraph_search(loop_paused)` devolvió solo 3 nombres de tests. codegraph no indexa strings.
- Conclusión: codegraph no modela almacenes. El par escritor→lector nunca aparece.

## Clase 2 — Aristas indirectas

### Método (reproducible)

- Función como valor: AST sobre cada `ast.Name` en contexto Load. El nombre debe ser una función top-level del mismo módulo o un `from X import f` de una función del repo. El padre debe ser keyword, argumento, default, Dict, List/Tuple/Set, Assign, Return o IfExp. Se descartan nombres sombreados por locales y listas `__all__`. Resultado: 104 sitios. Es un piso: no cuenta `mod.func` como valor ni lambdas.
- Decoradores: `decorator_list` de cada def, sin los de stdlib (`staticmethod`, `property`, `wraps`, `lru_cache`, `contextmanager`, `dataclass`).
- Despacho por string: `getattr` con segundo argumento no constante, `importlib.import_module`, y llamadas cuyo `func` es un Subscript (`tabla[k](...)`).
- Alias: `grep -rnE "^\s*from \S+ import .*\bas\b" mmorch scripts --include=*.py` da 84 líneas.

### Desglose de función como valor (104)

- 31 handlers de rutas Starlette: server.py:773-804 (`Route("/health", health_handler)` y otros).
- 21 checkers en el registro: checkers.py:572-592 (`_REGISTRY`).
- 13 checks de smoke en una lista: smoke.py:173-179.
- 10 kwargs que inyectan jobs y callbacks, por ejemplo server.py:208 `_run_rubric_job` y auto_apply_nightly.py:182 `_restart_from_env`.
- 8 etapas SDLC en un dict: sdlc.py:1394 `stages = {1: aceptacion, ..., 6: pr}`.
- 4 defaults de parámetro: auto_apply_nightly.py:163 (`collect_signals`), auto_apply_nightly.py:257 (`run_cycle`), sessions.py:230 (`record_outcome`, `cynefin_classify`).
- 17 restantes: asignaciones, returns e IfExp. Algunos son autotests internos (evolve.py:842).

### Ejemplos

- `auto_apply_nightly.py:257 run_nightly(..., cycle_fn=run_cycle)` y `:163 run_cycle(..., signal_fn=collect_signals)`. Son dos saltos de inyección seguidos. `nightly.py:620` importa `run_nightly as _run_auto_apply`: suma un alias.
- `checkers.py:609 _REGISTRY[name](**ctx)`: el oráculo checkeable despacha por string.
- `sdlc.py:1398 stages[k]()`: las etapas y gates del SDLC se llaman por dict.
- `mcp_server.py:264 _TOOL_RISK.get(name)`: el nombre de la función es clave string de la matriz de riesgo.
- `plugin_worker.py:74 getattr(mod, method)` y `plugins.py:203 host_services[hmethod](...)`: RPC por nombre entre procesos.

### Prueba con codegraph (la afirmación "sigue callbacks" es falsa en Python)

- `codegraph_callers(collect_signals)`: "No callers found".
- `codegraph_trace(run_nightly → collect_signals)` y `codegraph_trace(run_cycle → collect_signals)`: "No direct call path". El tool atribuye el corte a despacho dinámico.
- `codegraph_impact(collect_signals)`: 2 símbolos, la función y su archivo.
- `codegraph_callers(run_cycle)`: ninguno. `codegraph_callers(run_nightly)`: 4 tests. Falta `nightly.main` porque el import usa alias.
- `codegraph_trace(check → _check_arithmetic)`: sin camino. `codegraph_impact(_check_arithmetic)`: 2 símbolos.
- `codegraph_callers(health_handler)`, `(spec_review)` y `(mmorch_fan_out)`: ninguno.
- Decoradores: la DB guarda 44 aristas `decorates` (tool → `_tool`). `codegraph_impact(_tool)` lista los 44 tools. `codegraph_callers(_tool)` devuelve 0. Nadie ve la arista real: el cliente MCP llama al tool por nombre.
- Alias: `canary.py:114 _record_outcome(...)` no aparece en `codegraph_callers(record_outcome)`. Los imports locales sin alias sí resuelven: `callers(distill_backlog)` incluye `nightly.main`.
- Precisión: codegraph también inventa aristas por homónimos. 303 aristas van a `cache.get` y casi todas son `dict.get`. 86 van a `projects.resolve` y el grep encuentra 7 usos reales. 71 van a `transcript_store.append` y el grep encuentra 1. Además `callees(collect_signals)` lista `cache.get` por un `dict.get`.

## Clase 3 — Aristas de ejecución

### Método (reproducible)

- `grep -rnE "subprocess\.(run|Popen|call|check_output|check_call)|asyncio\.create_subprocess|os\.system\(" mmorch scripts --include=*.py`: 86 sitios de subproceso. La mayoría lanza git, pytest o el CLI de claude.
- Rutas a scripts o módulos: `grep -rnE "scripts[/\\][a-z_]+\.py|\"-m\", *\"mmorch"` sobre `mmorch`, `scripts`, `*.cmd` y `*.ps1`.
- Hooks: `~/.claude/settings.json` y `grep -lE "python|orchestration" ~/.claude/hooks/*.js`.
- Tareas programadas: `grep -rn "schtasks\|Register-ScheduledTask"`.
- Entrypoints: `[project.scripts]` de pyproject.toml, `mcpServers.mmorch` de `~/.claude.json` y `.pre-commit-config.yaml`.

### Conteo (~26 concretas)

- 12 subprocesos a código del repo: auto_apply_nightly.py:49 (pytest), :50 (`scripts/gates.py`), :51 (`scripts/smoke.py`); hardening.py:118 (`scripts/gate_hardening.py`); nightly.py:183 y :185 (scorers, vía `MMORCH_AR_SCORER`); plugins.py:152 (`-m mmorch.plugin_worker`); runtime_checkout.py:56 (`-m mmorch.nightly`); gates.py:21 (pytest de test_paths) y :23 (`-m mmorch.docgen`); nightly.py:498 (`schtasks /Run mmorch-server`); manana.py:142 (`importlib` de `veredicto`).
- 5 hooks de Claude Code: `autoregister_project.py` (SessionStart), `proposal_hook.py` (SessionStart startup), `session_ingest_hook.py` (SessionEnd), y dos JS que corren `python -m mmorch.context_blocks` (Stop y SessionStart compact).
- 3 tareas programadas: `mmorch-nightly` (nightly_task.cmd → scripts/nightly.py → `mmorch.nightly.main`), `mmorch-autopull` (autopull.cmd → `-m mmorch.sync pull-all`), `mmorch-server` (server_forever.ps1).
- 4 entrypoints: `mmorch-server`, `mmorch-sync` y `mmorch-mcp` en pyproject, más el MCP registrado en `~/.claude.json` que corre `mcp_server.py`.
- 2 pre-commit: ruff y mypy.
- Además hay 64 archivos con `if __name__ == "__main__"` (41 en mmorch, 23 en scripts). Son entradas potenciales sin llamador conocido.

### Caso real: arista rota en silencio

- El commit 19601ef (2026-09-14) borró `mmorch/context_blocks.py` con el motivo "sin entrada, sin tests, sin uso".
- Dos hooks lo siguen llamando: `~/.claude/hooks/context-block-reinject.js:17` y `context-block-watch.js:20`.
- Los hooks descartan stderr y fallan abiertos (`catch (_) { /* fail open */ }`). Nadie vio el error.
- `logs/context_blocks.db` dejó de cambiar el 2026-09-14 a las 13:55. El commit es de las 14:02.
- `tools/dead-modules.py` no puede verlo: solo escanea el repo, y los hooks viven en `~/.claude/hooks`.

### Prueba con codegraph

- `codegraph_search(context_blocks)` devuelve solo `_content_blocks` de sessions.py. codegraph no indexa los hooks JS ni los strings `-m`.
- `codegraph_callers(_cli)` devuelve `sync.py (file)` por el bloque `__main__`. No devuelve autopull.cmd ni la tarea programada.
- `codegraph_callees(collect_signals)` lista `_run` y `_json_run`. No lista gates.py ni smoke.py, que esos helpers lanzan.
- `codegraph_search(main)` junta 49 funciones `main`. Sin archivo, `callees(main)` mezcla todas.

## Clase 4 (extra) — Aristas que codegraph pierde por desactualización

- La DB muestra 0 aristas `calls` entrantes desde otros archivos hacia metrics.py, budget.py, learn.py y sdlc.py.
- Esos cuatro archivos se reindexaron entre 16:50 y 16:51. Sus llamadores (providers.py, patterns.py y otros) se indexaron antes, a las 13:12 o 14:41.
- Hipótesis: al reindexar un archivo, sus nodos cambian de id y se pierden las aristas entrantes de archivos no reindexados.
- Evidencia: el mapa registra 54 escritores para `log_event` más temprano. Hoy `codegraph_callers(log_event)` da 0. También dan 0 o solo llamadores internos: `build_feature` (4 llamadas reales), `monthly_spend` (sin budget_policy.py:55) y `remaining` (sin evolve.py:221).
- Esto toca el techo de gasto: `providers.py:276` importa `budget.check`, y codegraph no lo muestra.
- Consecuencia para el ticket 01/05: la línea base "codegraph solo" depende del estado del índice. Hay que reindexar completo antes de medir recall. Si no, la comparación favorece a la herramienta nueva por un bug del índice, no por mérito.

## Prioridad para tickets 04 y 05

1. **Datos (ticket 05, primero).** Tiene el mayor riesgo. `metrics.jsonl` alimenta el techo de gasto (`budget.monthly_spend`). `loop_paused` es el kill-switch y hoy parece escrito en otra carpeta. `nightly.jsonl` alimenta a auto_repair y project_repair, que modifican código solos. El contrato son claves string sin dueño. La extracción barata alcanza: strings de ruta y nombres de tabla más `sys.addaudithook`.
2. **Indirectas (ticket 04).** Tiene el mayor conteo: ~104 sitios de función como valor, 67 decoradores, 12 despachos por string y 84 alias. Toca gates: el registro de checkers (oráculo checkeable), las etapas del SDLC y `collect_signals`, que decide promociones de auto-apply. El AST estático las encuentra casi todas; el grep por palabra es la línea base.
3. **Ejecución (ticket 05, después).** Tiene pocas aristas (~26), pero se rompen en silencio: el hook de context_blocks falló abierto 10 días. Un scan de strings sobre hooks, `.cmd`, `.ps1`, settings y pyproject alcanza.
4. **Transversal.** Reindexar codegraph completo antes de medir, y resolver imports con alias. Sin eso, la línea base de codegraph queda subestimada y la regla anti-scope-creep mide mal.

## Lo que no verifiqué

- La divergencia de raíces de `smoke.jsonl` bajo `MMORCH_HOME` es una hipótesis. No corrí el rollout.
- Si `MMORCH_MAX_MONTHLY_USD` existe en el entorno del usuario. En el `.env` del repo no está.
- Si el `loop_paused` en `logs/auto_apply/` es intencional.
- El conteo de pares escritor→lector (~75-80) es manual y aproximado. El de función como valor (104) es un piso.
- La causa de la clase 4 es una hipótesis basada en los tiempos de indexado. No la reproduje con un reindex.
