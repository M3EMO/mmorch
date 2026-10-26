# La skill /project pasa los archivos de la tarea
Type: task
Status: resolved
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

## Answer

Hecho el 2026-10-24, junto con la implementación de los tickets 01 a 10.

- La skill `~/.claude/skills/project/SKILL.md` pide siempre `files`. Primero los saca del texto de la tarea, después de la conversación, y si no aparecen pregunta.
- Antes de lanzar, la skill lee `sdlc.toml`. Con `[areas]` y sin archivos ni rutas en el texto, avisa y pide los archivos.
- La skill muestra en la parada única los tests de las etapas 0 y 1, los movimientos a `removidos/` y las bajas.
- La skill explica el corte `0-areas` con `division.md` y los dos commits de la etapa 6.
- El servidor ya pasaba `files` a `build_feature`; no cambió. El run-log nombra el área en la clave `area`.
- La skill no vive en ningún repo git: el cambio queda solo en `~/.claude/skills/project/SKILL.md`.
