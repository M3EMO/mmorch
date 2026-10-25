# Repos-convenciones, ticket 05 (valores por máquina): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir dónde viven los valores de `sdlc.toml` que solo valen en una máquina (`.scratch/repos-convenciones/issues/05-valores-por-maquina.md`, bd `orchestration-4u6`).

Hechos (2026-10-23):
- Búsqueda de rutas absolutas en los diez `sdlc.toml`: un solo caso, `suite` de orchestration con `--basetemp C:/Users/map12/AppData/Local/Temp/pyt-sdlc-wt`.
- Ese valor entró en `fd33b1b` (2026-09-15). El mismo commit puso en `build_feature` un default portátil: `suite` = `["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", tempfile.gettempdir()/"pyt-sdlc-wt"]` (hoy `sdlc/pipeline.py:1968`). El toml congeló la expansión Windows de ese default.
- `mmorch/health.py:286` y `mmorch/evolve.py:401` explican el `--basetemp`: el `pytest-current` global de Windows queda con permisos rotos.
- El escritorio Windows corre el server de mmorch, así que lee los mismos `sdlc.toml` que la laptop.

## Summary / key decisions

- Test de aceptación aprobado con 11 puntos (P3). Ticket 05 resuelto.
- Partición de textos: comandos de texto con `shlex.split`; listas por elemento; `=` se parte solo en pedazos que empiezan con `-`. Patrones: letra de unidad en cualquier caso, `/`, `~`, `\\\\` (P3).
- Ruta absoluta = texto que empieza con letra de unidad (`C:/`, `C:\\`), con `/` o con `~`. `doctor` revisa cada texto, también dentro de listas y tablas (P2).
- Valores por máquina: opción C. `doctor` marca como desvío una ruta absoluta en `sdlc.toml`. La meta del usuario: "no tener rutas absolutas". La opción B (variables como `{tmp}`) queda como respaldo si aparece un caso real que la necesite (P1).
- orchestration: borrar la clave `suite` a mano (el default del código da la misma suite portátil). `--upgrade` no la borra, porque borrar contenido no está en la lista cerrada del ticket 03.

## Q&A log

### P1 — Mecanismo para valores por máquina
- Asked: ¿A (`sdlc.local.toml` ignorado), B (variables como `{tmp}`), C (desvío por ruta absoluta) o D (nada)?
- Captured: el usuario respondió "C. La idea es no tener rutas absolutas. De ultima B".
- Decisión: C ahora; B solo si aparece un caso que C no resuelva.

### P2 — Qué cuenta como ruta absoluta
- Asked: ¿confirmas los tres patrones (letra de unidad, `/`, `~`)?
- Captured: el usuario confirma.
- Nota: `~` entra porque apunta a una carpeta distinta en cada máquina. (Claude primero dijo que el pipeline corre sin shell; eso vale solo para listas como `suite`. Los comandos de texto corren con shell, según `sh()`. Corregido en P3.)

### P3 — Test de aceptación
- Asked: ¿apruebas el test de 7 puntos con la partición en espacios y `=`? El usuario preguntó si faltaba algo.
- Captured: Claude corrigió su argumento de P2 sobre el shell y agregó los puntos 8 (`shlex.split`), 9 (`=` solo en pedazos con `-`), 10 (unidad en minúscula) y 11 (ruta de red `\\\\`). Preguntó si `~` sigue como desvío. El usuario respondió "sí, apruebo el test con los puntos 8 a 11".
- Decisión: test aprobado; `~` sigue como desvío.

## Open flags (pending input)
- Borrar `suite` del `sdlc.toml` de orchestration: a mano por el usuario, o por un agente con su OK.
