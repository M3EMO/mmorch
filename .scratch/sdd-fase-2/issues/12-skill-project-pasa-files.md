# La skill /project pasa los archivos de la tarea
Type: task
Status: open
Blocked by: 01
Map: ../map.md

## Question

¿Cómo llega `--files` a cada corrida que lanza la skill `/project`?

Origen: verificación del ticket 01 (V1, 2026-10-23). La skill `/project` (`~/.claude/skills/project/SKILL.md`) trata `--files` como opcional. Con `sdlc.toml`, `_resolve_files()` devuelve el techo y `FEAT["files"]` queda vacío. Sin `files`, la corrida no tiene área ni delta.

Decisión del usuario en el ticket 01: el área sale de los `files` del llamador. Sin `files`, el respaldo determinista lee las rutas del texto de la tarea.

Puntos a resolver al implementar:

- La skill arma `--files` desde la tarea, desde la conversación o preguntando al usuario.
- La skill avisa cuando el repo declara áreas y la tarea no trae `files` ni rutas en el texto.
- `sdlc/builder.py` ya pasa `files` siempre y no cambia.

## Test de aceptación propuesto

Una corrida de `/project` sobre un repo con áreas declaradas llega a `build_feature` con `files` no vacío, o con una ruta en el texto de la tarea. El run-log de esa corrida nombra el área resuelta. Una corrida sobre un repo sin áreas no cambia respecto de hoy.
