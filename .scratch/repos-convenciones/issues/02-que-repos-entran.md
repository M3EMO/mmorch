# Qué repos entran y dónde vive la lista
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md

## Question

¿Qué repos actualiza el mecanismo y qué archivo es la fuente de verdad de esa lista?

El pedido nombra cinco repos: orchestration, ChatBot, Estudio, Portfolio y Proyecto_Adepor. La sonda encontró diez repos con `sdlc.toml` en `/home/mpera`. Adepor no tiene `sdlc.toml`, y el usuario lo dejó fuera del SDLC el 2026-09-17. `projects.json` de mmorch no lista Portfolio, ChatBot ni ssb, y sí lista worktrees y carpetas `scratch-*`.

Opciones a decidir:

- A. Descubrimiento: un repo entra si tiene `sdlc.toml` bajo una raíz declarada (por ejemplo `/home/mpera`). La lista no se mantiene a mano.
- B. Registro: `projects.json` gana un campo `sdlc = true`, y el mecanismo solo recorre esas entradas. Hace falta limpiar el registro.
- C. Lista explícita: un archivo nuevo en orchestration nombra cada repo. El usuario la edita a mano.
- D. Sin lista: el mecanismo actúa sobre un repo por invocación, y el usuario elige cuál.

Preguntas derivadas:

- ¿Los prototipos de `F2-seo-calculadoras` entran, o son repos de usar y tirar?
- ¿Adepor entra a las costumbres SDD (specs) sin entrar al SDLC? Hoy tiene `docs/specs/` con otra forma (ticket 06).
- ¿El escritorio Windows cuenta? Sus clones pueden tener commits que la laptop no tiene (los commits de adopción de Adepor no están acá).

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_lista_de_repos`: en `tmp_path` se arman tres repos git, uno con `sdlc.toml`, uno sin `sdlc.toml` y uno dentro de `.claude/worktrees/`. La función que arma la lista devuelve solo los repos que la opción elegida incluye, en orden estable. Un worktree nunca entra.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-02.md`.

- `doctor <repo>` y `--upgrade <repo>` actúan sobre un repo. La bandera `--todos` descubre los repos adoptados bajo `Path.home()`.
- `sdlc.toml` es la fuente de verdad de la adopción. No hay lista aparte, y `projects.json` no se usa para esto.
- Entra cada carpeta con un `.git` propio que sea directorio y con `sdlc.toml`. Un `.git` archivo (worktree o submódulo) no entra, y las carpetas que empiezan con punto tampoco.
- `/home/mpera/Proyectos` es un repo git con repos anidados (ChatBot, mf-moe, ssb, F2). Los repos anidados entran.
- No hay mecanismo de exclusión. Un prototipo descartado se borra o se archiva.
- Adepor (`~/BetDeportivo`) entra con `sdlc.toml`, y eso revierte la decisión del 2026-09-17. `python -m sdlc init ~/BetDeportivo` corre dentro del ticket 07.
- La laptop es la única máquina que corre `--upgrade`, como regla escrita y sin chequeo en código. El escritorio recibe los cambios por pull.
- `doctor` avisa si una rama local está detrás de su ref remota, sin red. Sin `git fetch` previo, el aviso es una pista y no una prueba.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_lista_de_repos` arma repos git en `tmp_path` y verifica diez puntos:

1. `--todos` devuelve solo los repos con `sdlc.toml`, ordenados por ruta.
2. Un repo con `sdlc.toml` anidado dentro de otro repo entra.
3. Un worktree con `sdlc.toml` y un `.git` archivo no entra.
4. Un repo dentro de una carpeta que empieza con punto no entra.
5. Un repo sin `sdlc.toml` no entra, y `doctor <ruta>` sobre él sale con un código propio.
6. `doctor` avisa si la rama local está detrás de su ref remota, sin llamar a la red.
7. Dos corridas de `--todos` dan la misma lista.
8. Un symlink a una carpeta no se sigue, así que un symlink en ciclo no cuelga el recorrido.
9. El recorrido salta `node_modules` y las carpetas sin permiso de lectura, sin lanzar un error.
10. Un repo con un `sdlc.toml` roto entra a la lista, y `doctor` lo marca como desvío.
