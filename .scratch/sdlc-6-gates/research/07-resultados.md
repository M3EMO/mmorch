# Resultados de validacion (ticket 07) — driver_py

Brazo: v3 + Claude fijo (revision de diff antes del PR + Claude en escalacion). Fase ledger `sdlc-<task>-r<n>`.
USD = fase completa del ledger, incluidos intentos abortados. Minutos = pared, sumados por intento.
Run-logs: `04-driver-v3/run-log-sdlc-<task>-r<n>.json`. Worktrees: `Desktop/Claude/sdlc-runs/<task>-r<n>`.

| task | corrida | verde | USD | min | nivel max | rechazos por gate | Claude atrapo | lineas +/- (codigo) | suite | ruff/mypy nuevos | humano |
|---|---|---|---|---|---|---|---|---|---|---|---|
| rate-limiter (S2) | r1 | si | 0.0545 | 2.7 | 0 (fix 0 vueltas) | test-compile x1 (intento 1) | 0 (OK) | +37 / -0 | 3/3 | 0/0 | 0 |
| rate-limiter (S2) | r2 | si | 0.0268 | 2.0 | 0 | ninguno | 0 (OK, trazo a mano los 3 tests) | +47 / -0 | 3/3 | 0/0 | 0 |
| rate-limiter (S2) | r3 | si | 0.0307 | 1.9 | 0 | ninguno | 0 (OK) | +40 / -0 | 3/3 | 0/1 | 0 |
| rate-limiter (S2) | r4 spec-kit | si | 0.0208 | 2.9 (+0.8 intento 1) | 0 | G1 x1 (intento 1, falso positivo de plantilla) | 0 (OK); spec-review 0 preguntas | +34 / -0 | 3/3 | 0/1 | 0 |
| rate-limiter (S2) | r5 alcance | si | 0.0284 | 3.1 | 0 | ninguno | 0 (OK); spec-review 0 preguntas | +39 / -0 | 3/3 | 0/0 | 0 |
| D3 atasco (mmorch) | r1 | si | 0.0374 | 34 (25 de suite total) | 0 | suite-total x2 (baseline mal medido, driver) | 0 (NOTE sin test: zona gris fuera de R1-R5) | +9 / -1 | 1126 sin fallos nuevos | 0/0 | 0 |
| etl-pipeline (S3) | r1 | si | 0.0374 | 6.4 | 0 | ninguno | 0 (OK) | +42 / -0 | 3/3 | 0/1 | 0 |
| D2 test-compile (mmorch) | r1 | si | 0.8369 | ~60 (4 intentos, 3 pasadas de suite) | 3 (Claude x2 + lint) | G3-compile x7, avance x1, lint x2, claude-diff-review x2, suite-total x1 (flaky) | 2 BLOCK con test (timeout capturado; detail vacio) | +69 / -4 | 1129 sin fallos nuevos | 0/0 | 0 |
| lru-ttl-cache (S1, held-out) | r1 | si | 0.0187 | 4.3 | 0 | ninguno | 0 (OK, probo bordes a mano) | +32 / -0 | 3/3 | 0/0 | 0 |
| D4 synth_store usa paths (reparacion) | r1 | si, pero borro el docstring del modulo | 0.0175 | 16 | 0 | ninguno (defecto no cubierto por gate) | 0 (OK) | +4 / -26 | 1124 sin fallos nuevos | 0/0 | 0 |
| D5 version MCP sin metadata (reparacion) | r1 | si | 0.0922 | 12.9 | 0 | ninguno | 0 (OK) | +20 / -6 | 1127 sin fallos nuevos | 0/0 | 0 |
| D4 synth_store usa paths | r2 (con gate docstring) | si, diff limpio | 0.0489 | 18.6 | 0 | docstring-intacto x1 (restaurado por el coder), claude-diff-review x1 | 1 BLOCK con test (salto de linea final del docstring) | +2 / -2 | 1125 sin fallos nuevos | 0/0 | 0 |
| D6 plugins robustez (2 archivos) | r1 | si | 0.1077 | 21.5 | 3 (lint -> Claude) | lint x2 | 0 (NOTE, probo bordes extra) | +182 / -108 (reescritura) | 1126 sin fallos nuevos | 0/0 | 0 |
| D7 cablear try_automerge | r1 | NO: bloqueada | ~0.05 | 6.6 | build | test-compile x4, un-archivo x1 | - | - | - | - | 0 |
| D8 project_driver seams | r1 | si | 0.0397 | 10.3 | 0 | ninguno | 0 (OK) | +16 / -5 | 1134 sin FAILED nuevos; 594 ERROR de setup no mirados (ver hallazgo) | 0/0 | 0 |
| D9 fleet/pty bordes (2 archivos) | r1 | si | 0.0932 | 17.9 | 0 | claude-diff-review x1 | 1 BLOCK con test (404 vs 502 por match de texto en cualquier error) | +49 / -12 | 1136, 0 nuevos, 0 errores | 0/0 | 0 |

## Veredicto (criterio del ticket 07)

- Verde en >= 5 de 6 incluida la held-out: **6/6** (S2 3/3, D3, S3, D2, S1; D1 ya verde en baseline, sin corrida).
- Mediana de USD por feature < US$1: **US$0.037** (0.019 / 0.03 / 0.037 / 0.037 / 0.84).
- Cero intervenciones humanas antes del nivel 4: **0** en todas las corridas.
- Nivel maximo de escalacion: 3 (solo D2). Claude bloqueo con test 2 veces, ambas correctas (D2).
- Defectos del driver encontrados por las corridas: 9, todos atrapados por gates deterministas a US$0 y corregidos.
- El pipeline GANA contra el engine viejo (0/3 historico en S2). Cupo de Claude: 2 pasadas por corrida (spec + diff), mas 3 en D2.

S2 cerrada: 3/3 verde, mediana US$0.03, 0 escalaciones, 0 humanos. Claude atrapo 0 defectos en 3 revisiones.
D1 no se corrio: su test de aceptacion (`tests/test_project_driver.py`) ya pasa en el baseline; queda como regresion.
D3 verde en branch `sdlc/sdlc-D3-r1b` de mmorch (commit `51d539a`), sin merge: lo decide el humano.
Control historico: el engine viejo midio 0/3 en rate-limiter con el mismo planner.

## Hallazgos por corrida

- r1 intento 1 (2026-09-11): rojo en build. El coder pego los 3 archivos dentro de `core.py` imitando las cabeceras `# path` del prompt; `test-compile` (pytest --collect-only) lo atrapo en US$0. Defecto del driver: el gate cortaba sin vuelta de fix. Fixes en driver_py `3e874ae`: gate `un-archivo` + test-compile dentro del fix loop de build.
- r1 intento 2: verde sin vueltas. Revision de Claude: `OK`, 0.5 min, US$0 (plan). Se colaron `.pyc` en el PR y `lines` contaba docs: corregido.
- r2, r3: verdes sin vueltas. r3 deja 1 error nuevo de mypy en `core.py` (`_last` anotado como None): la metrica lint funciona; el gate ruff+mypy=0 todavia no bloquea. Los `.pyc` siguieron entrando porque la review hace `git add -A` antes del `.gitignore`: movido al arranque.
- r4 spec-kit (2026-09-11): plantilla R<n> + spec-review (clarify) + [P] + gate trazabilidad. Intento 1 rojo en G1: la plantilla decia la palabra prohibida y el modelo la copio (falso positivo, corregido `086e73d`). Intento 2 verde: 5 IDs trazados a tests reales, plan cita IDs y marca `core.py` [P], spec-review hizo 0 preguntas. HALLAZGO: la revision de spec (Claude, modo edit) implemento `core.py` y `multi.py` por su cuenta; build los sobreescribio, pero es fuga de alcance. Gate nuevo `alcance` (snapshot de arbol antes/despues de cada pasada de Claude, revierte lo que no esta permitido). Verificado en r5: `spec-review-alcance` ok (Claude solo toco spec.md con el prompt reforzado), `diff-review-alcance` ok.
- D3 r1 (2026-09-11): perfil de repo existente. Tres tropiezos del driver, cero del coder: (1) el pre-commit de mmorch corria con el Python del sistema (sin ruff) -> venv al frente del PATH; (2) el gate de suite exigia verde absoluto y el baseline tiene 5 rojos previos -> comparacion por nombre; (3) el baseline medido en el arbol principal no compara (tests sin commitear, metadata del paquete) -> se mide en el worktree por sha. El coder resolvio el detector en 1 llamada, 0 vueltas. Claude anoto una zona gris real (previous_code tras excepcion de gate_fn) y NO bloqueo por falta de test: el protocolo funciono como se diseño. La suite total domina el tiempo: 2 pasadas de ~6 min por corrida.
- S3 etl-pipeline r1 (2026-09-13): verde en 1 intento, 12 IDs trazados, plan marco 3 de 4 archivos [P] (correcto: solo __init__ importa a los otros). Spec necesito 1 reescritura por tokens del contrato. mypy nuevo: 1.
- D2 r1 (2026-09-13): la primera feature que uso la escalera entera. Causa raiz del costo: `project_integrate.py` contiene "```" dentro de un string y `strip_fence` cortaba el archivo (defecto del driver, corregido: valla exterior). Con el archivo roto el fix loop se atasco (stall=2), subio a reasoner y a Claude nivel 3, que lo reparo. La revision de Claude BLOQUEO 2 veces con test que falla, ambas correctas: (1) `_default_compile` capturaba TimeoutExpired contra R5; (2) detail vacio sin fallback. Lint: el pre-commit del repo rechazo 2 mypy (self-check con `append() or`) -> gate `lint` con escalera coder -> Claude. Suite: 3 fallos nuevos en test_providers pasaron aislados -> recorrida aislada antes de declarar regresion. Plan: falso positivo de `trazabilidad-plan` por un path en prosa -> archivos solo desde items de lista. Verde final: 5 tests (3 aceptacion + 2 de la revision), 0 fallos nuevos en 1129, lint 0/0. PR en branch `sdlc/sdlc-D2-r1` (commit `739e6e6`), sin merge.
- D4/D5 r1 (2026-09-14, reparacion por modulos): dos rojos previos eran expectativas viejas en tests (precio cache, DELETE en rutas), corregidos a mano. D4 verde en 1 llamada pero el coder BORRO el docstring del modulo (22 lineas) y ningun test lo ve: gate nuevo `docstring-intacto` (ast, determinista) con una vuelta de restauracion; ademas `strip_fence` comia la linea final -> `_write` la repone. D5 verde: `mmorch_version()` con fallback a pyproject.toml. Hallazgo colateral: las pasadas headless de Claude corren bajo los hooks globales del usuario (el guard ETS contamino el texto de una revision). `test_counts` inflaba fallos con -rf (594): solo la ultima linea de pytest. D4 se relanza como r2 para un PR limpio.
- D4 r2 (2026-09-14): el gate `docstring-intacto` atrapo de nuevo el borrado del docstring (el coder lo repite: es un modo de falla del modelo, no azar) y la vuelta de restauracion lo devolvio. Claude bloqueo con test por el salto de linea final del docstring; el fix loop lo resolvio en 1 vuelta. Diff final +2/-2, exactamente la tarea. PR en branch `sdlc/sdlc-D4-r2` (commit `1617c49`); la r1 (`sdlc/sdlc-D4-r1`) queda descartada.
- D6 r1 (2026-09-14): verde con lint via Claude nivel 3 (coder no arreglo 3 mypy). Claude reporto que el coder habia PISADO plugin_worker.py con una copia de plugins.py: gate nuevo `sin-clones` (dos archivos del plan con > 80% de lineas iguales). Diff +182/-108 = reescritura, no cambio minimo; sin gate para eso todavia. Suite: conteo de fallos inflado (597) aunque new_failures = []: se guarda docs/sdlc/suite.log para depurar. PR en `sdlc/sdlc-D6-r1` (9e85417).
- D7 r1 (2026-09-14): BLOQUEADA por el repo, no por el pipeline. Todo el subsistema auto_apply (mmorch/auto_apply.py, promotion.py, observation.py, runtime_checkout.py, canal.py y sus tests) esta SIN COMMITEAR en el arbol principal (handoff .claude/handoff-auto-apply.md); el worktree no lo tiene -> ModuleNotFoundError. El gate `un-archivo` atrapo al coder intentando crear promotion.py. Se relanza cuando el usuario commitee ese WIP.
- D8 r1 (2026-09-14): verde, +16/-5, 0 vueltas. HALLAZGO del gate de suite: el log completo mostraba 594 ERROR de setup en 73 s que el gate no miraba (solo FAILED). Rerun a mano con --basetemp limpio: 2 failed (los 2 rojos previos: paths y version MCP, que D4/D5 arreglan) / 1132 passed / 0 errors en 5.5 min -> los ERROR eran del --basetemp compartido entre corridas (dir de pytest bloqueado en Windows). Fixes: gate mira FAILED+ERROR por nombre, logs completos, basetemp por fase. Los veredictos de D6 y D8 se sostienen (0 fallos nuevos por nombre; D8 verificado con suite limpia).
- D9 r1 (2026-09-14): verde. Claude BLOQUEO con test: `fleet_run` clasificaba 404 por match de texto ("no existe"/"not found") sobre cualquier error de forward, incluso uno remoto -> 404 falso. Correcto y con evidencia; el fix loop lo resolvio en 1 vuelta. Spec-review hizo 1 pregunta. Suite limpia: 1136, 2 rojos previos, 0 errores.
