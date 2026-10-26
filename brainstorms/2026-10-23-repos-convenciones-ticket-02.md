# Repos-convenciones, ticket 02 (qué repos entran): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué repos actualiza el mecanismo y qué archivo es la fuente de verdad de esa lista (`.scratch/repos-convenciones/issues/02-que-repos-entran.md`, bd `orchestration-4u6`).

Contexto heredado: ticket 03 decidió que `--upgrade` nunca crea `sdlc.toml` (un repo sin `sdlc.toml` no está adoptado). La sonda del 2026-10-23 encontró diez repos con `sdlc.toml` en `/home/mpera`. `projects.json` no lista Portfolio, ChatBot ni ssb, y trae worktrees y carpetas `scratch-*`.

## Summary / key decisions

- Test de aceptación aprobado con 10 puntos (P6). Ticket 02 resuelto.
- La laptop es la única máquina que corre `--upgrade`; el escritorio recibe los cambios por pull. `doctor` avisa si una rama local está detrás de su remoto según refs locales, sin red (P5).
- Corrección de P1: `/home/mpera/Proyectos` es un repo git, y ChatBot, mf-moe, ssb y los F2 son repos anidados dentro. La regla "salta carpetas dentro de otro repo" los dejaría fuera. Regla corregida: entra cada carpeta con `.git` propio (directorio) y `sdlc.toml`; un `.git` archivo (worktree) y las carpetas que empiezan con punto no entran.
- `python -m sdlc init ~/BetDeportivo` corre dentro del ticket 07, con su regla de rama, commit y árbol sucio (P4).
- Adepor (`~/BetDeportivo`) entra: recibe `sdlc.toml`. Revierte la decisión del 2026-09-17 que lo dejaba fuera del SDLC (P3).
- Sin mecanismo de exclusión: un prototipo descartado se borra o se archiva; un prototipo vivo recibe las convenciones (P2).
- Fuente de la lista: opción D más A. `doctor <repo>` y `--upgrade <repo>` actúan sobre un repo; una bandera `--todos` descubre repos con `sdlc.toml` bajo `/home/mpera`, y salta worktrees y carpetas dentro de otro repo. `sdlc.toml` es la fuente de verdad de la adopción (P1).

## Q&A log

### P1 — Fuente de la lista de repos
- Asked: ¿A (descubrir por `sdlc.toml`), B (`projects.json` con `sdlc = true`), C (lista a mano) o D (un repo por invocación)?
- Captured: el usuario confirma "D más A".
- Decisión: sin lista aparte; `projects.json` no se usa para esto.

### P2 — Excluir un repo con `sdlc.toml`
- Asked: ¿A (sin exclusión; borrar o archivar el prototipo), B (clave `convenciones = false`) o C (lista de exclusión)?
- Captured: el usuario confirma A.
- Decisión: los cuatro repos de `F2-seo-calculadoras` entran a `--todos` mientras tengan `sdlc.toml`.

### P3 — Adepor
- Asked: ¿A (fuera de todo hasta adoptarlo con `init`) o B (`doctor` parcial sin `sdlc.toml`)?
- Captured: el usuario respondió "Incluir SDLC toml a adepor": ninguna de las dos; Adepor se adopta.
- Hechos (sonda de solo lectura, 2026-10-23): `pytest.ini` tiene `testpaths = tests`, así que pytest ya no recolecta los scrapers de `analisis/` (el bloqueo de septiembre). `tests/` tiene 31 archivos `test_*.py`. `tests/test_ingesta_cuotas.py` usa `requests` (sin verificar si con mock). Los commits del escritorio `1d5c71e` (sdlc.toml) y `b197ffe` (oráculo `test_oraculo_motor.py`) no existen en este clon. Rama actual `audit/prod-ready-2026-10`, con 2 archivos modificados.
- Consecuencia: el ticket 06 conserva el caso de las specs planas de Adepor.

### P4 — Cuándo adoptar Adepor
- Asked: ¿A (ahora, tras una sonda sin red) o B (en el ticket 07)?
- Captured: el usuario eligió B.

### P5 — Clones del escritorio
- Asked: ¿A (cada máquina sobre sus clones), B (solo la laptop corre `--upgrade`) o C (aviso de rama detrás del remoto, sin red)?
- Captured: el usuario eligió "B más C".
- Decisión: sin `git fetch` previo las refs pueden estar viejas; el aviso es una pista, no una prueba.
- Hallazgo al preparar el test: `/home/mpera/Proyectos` es un repo git con repos anidados. Claude corrige la regla de P1 (ver Summary) y lo informa al usuario.

### P6 — Test de aceptación
- Asked: ¿apruebas el test de 7 puntos con la regla corregida? El usuario preguntó si faltaba algo.
- Captured: Claude agregó los puntos 8 (sin seguir symlinks), 9 (salta `node_modules` y carpetas sin permiso) y 10 (`sdlc.toml` roto entra y es desvío), y propuso aceptar que un submódulo no entra. El usuario aprobó "el test con los puntos 8 a 10".
- Decisión: test aprobado. El límite de los submódulos iba en la misma pregunta; el usuario no lo nombró aparte.

## Open flags (pending input)
- Límite de submódulos (un `.git` archivo no entra): aprobado junto con el test, sin mención explícita -> usuario puede objetar.
- ¿`tests/` de Adepor corre sin red? `accept_cmd` debe ser determinista -> sonda del ticket 07 antes de adoptar.
- ¿`--upgrade --todos` escribe en lote, o solo `doctor --todos`? -> ticket 07.
