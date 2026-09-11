# Resultados de validacion (ticket 07) — driver_py

Brazo: v3 + Claude fijo (revision de diff antes del PR + Claude en escalacion). Fase ledger `sdlc-<task>-r<n>`.
USD = fase completa del ledger, incluidos intentos abortados. Minutos = pared, sumados por intento.
Run-logs: `04-driver-v3/run-log-sdlc-<task>-r<n>.json`. Worktrees: `Desktop/Claude/sdlc-runs/<task>-r<n>`.

| task | corrida | verde | USD | min | nivel max | rechazos por gate | Claude atrapo | lineas +/- (codigo) | suite | ruff/mypy nuevos | humano |
|---|---|---|---|---|---|---|---|---|---|---|---|
| rate-limiter (S2) | r1 | si | 0.0545 | 2.7 | 0 (fix 0 vueltas) | test-compile x1 (intento 1) | 0 (OK) | +37 / -0 | 3/3 | 0/0 | 0 |
| rate-limiter (S2) | r2 | si | 0.0268 | 2.0 | 0 | ninguno | 0 (OK, trazo a mano los 3 tests) | +47 / -0 | 3/3 | 0/0 | 0 |
| rate-limiter (S2) | r3 | si | 0.0307 | 1.9 | 0 | ninguno | 0 (OK) | +40 / -0 | 3/3 | 0/1 | 0 |

S2 cerrada: 3/3 verde, mediana US$0.03, 0 escalaciones, 0 humanos. Claude atrapo 0 defectos en 3 revisiones.
Control historico: el engine viejo midio 0/3 en rate-limiter con el mismo planner.

## Hallazgos por corrida

- r1 intento 1 (2026-09-11): rojo en build. El coder pego los 3 archivos dentro de `core.py` imitando las cabeceras `# path` del prompt; `test-compile` (pytest --collect-only) lo atrapo en US$0. Defecto del driver: el gate cortaba sin vuelta de fix. Fixes en driver_py `3e874ae`: gate `un-archivo` + test-compile dentro del fix loop de build.
- r1 intento 2: verde sin vueltas. Revision de Claude: `OK`, 0.5 min, US$0 (plan). Se colaron `.pyc` en el PR y `lines` contaba docs: corregido.
- r2, r3: verdes sin vueltas. r3 deja 1 error nuevo de mypy en `core.py` (`_last` anotado como None): la metrica lint funciona; el gate ruff+mypy=0 todavia no bloquea. Los `.pyc` siguieron entrando porque la review hace `git add -A` antes del `.gitignore`: movido al arranque.
