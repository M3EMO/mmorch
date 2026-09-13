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
