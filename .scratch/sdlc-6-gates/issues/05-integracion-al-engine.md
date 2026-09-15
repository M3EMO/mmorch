# Integracion al engine
Type: grilling
Status: closed (2026-09-15)
Blocked by: 02, 03, 04
Map: ../map.md

## Question

El pipeline reemplaza a project-build (decisión del usuario). Decidir qué se reemplaza y qué se conserva: `project_driver.py` (recursión por unidades) y `project_integrate.py` (coder, cold verifier, integrate) vs el driver por etapas; se conservan worktree aislado, checkpoints, review branch, `/run/workflow`, Lotus. Cómo migran los 4 fixes ya hechos (regla de unidad de aceptación, deps al coder, gen_model/max_fix por payload, max_tokens 32768). Cómo se retira el engine viejo sin romper `tests/test_project_*` ni el skill `/project`. Plan de migración en pasos con test cada uno.

## Resolucion (2026-09-15, HITL, 4 decisiones aceptadas por el usuario)

- D1 (usuario acepta): el driver entra a mmorch como `mmorch/sdlc.py` con `build_feature(task, repo, accept_cmd, files, ...)`.
  `server_engine` (`/run/project`) y `workflow_race` lo llaman en lugar de `project_integrate.build_project`.
  `project_driver.py` y `project_integrate.py` se borran con sus 4 tests. Consumidores reales medidos: solo esos dos
  (evolve, evolve_findings y project_build nombran el archivo en listas de rutas, no lo importan).
- D2 (usuario elige C): la lista de archivos permitidos sale de `sdlc.toml` (techo por modulo, revisado una vez);
  el payload solo la ACOTA, nunca la amplia; sin lista en el payload aplica la del toml completa. Un cableo que
  toca `tests/test_capas.py` lo declara en el toml (visible en el diff).
- D3 (usuario acepta): retiro en 4 pasos con test en cada uno: (1) `mmorch/sdlc.py` con `build_feature` + tests
  unitarios de los gates puros (strip_fence, cambio-minimo, un-archivo, trazabilidad, allowlist); (2) `server_engine`
  y `workflow_race` cambian a `build_feature`, tests ajustados; (3) se borran `project_driver.py`,
  `project_integrate.py` y sus 4 tests; (4) el skill `/project` conserva el NOMBRE y cambia el cuerpo.
  Los 4 fixes del engine viejo ya viven en el driver: no escribe tests (plan-allowlist), pasa todos los archivos
  del plan al coder, modelo/max_fix como parametros de `build_feature`, max_tokens 32768.
- D4 (usuario acepta): la ETAPA es el checkpoint. `run-log.json` + `--from-stage` reemplazan al checkpoint por
  unidad; el resume del server llama a `build_feature(from_stage=...)`. El store de bloques deja de recibir
  escrituras del engine; no se borra todavia (queda en la niebla: retiro de blocks cuando nadie lo lea).

Siguiente: los 4 pasos son ejecucion, no decision. Se construyen con el propio pipeline (dogfood: D16 = paso 1).

## Ejecucion

- Paso 1 HECHO (2026-09-15): `mmorch/sdlc.py` (1001 lineas, port mecanico del driver: configure() + run() +
  build_feature() + main()), plantillas en `mmorch/sdlc_templates/`, `tests/test_sdlc_gates.py` (8 tests de gates
  puros, cada uno de un defecto real), `sdlc` registrado como entrada CLI en test_capas, excluido del selfcheck
  (corre API real). `driver_py.py` queda como CLI fino con la tabla FEATURES. Sin `files`, build_feature lee
  `sdlc.toml` de la raiz del repo (D2). Baseline de suite y casos de diagnostico van a `logs/sdlc/`.
- Paso 2 HECHO (b553ebd): `server_engine._run_project_build_job`, `workflow_race._default_build_fn`,
  `project_repair`, `auto_repair` y `hardening` llaman a `sdlc.build_feature`; fallos como `StageFailed`
  (el server los mapea: built -> done, etapa 5 -> gate, otro -> escalate); `accept_cmd` como oraculo cuando
  el comando no nombra tests; `files` = techo de `sdlc.toml` acotado por el payload (`_resolve_files`);
  resume por `resume_branch` + `from_stage` en el mismo payload de /run/workflow.
- Paso 3 HECHO (bd014a1): borrados project_driver, project_integrate, project_build y lang (quedaron sin
  consumidor) con 6 tests; piso de selfchecks 40 -> 37. mmorch: 122 -> 118 modulos.
- Paso 4 HECHO: `~/.claude/skills/project/SKILL.md` describe el pipeline, el payload nuevo (files, max_fix,
  resume_branch/from_stage), los estados terminales y la regla "el pipeline nunca escribe tests". Nombre `/project` intacto.
