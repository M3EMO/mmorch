# Protocolo: asignar, despertar, devolver y verificar
Type: grilling
Status: resolved
Blocked by: 03
Map: ../map.md

## Question

¿Cómo asigna Claude (bd assignee, spec del ticket), cómo se despierta a Cursor (despachador a demanda o por agenda), cómo devuelve Cursor (rama del worktree + `handoff` en el canal) y qué verifica Claude antes de integrar (oráculo, gates de SDLC)?

## Notes

- Decidido en "Dónde entra Cursor" (2026-09-30): permisos por `<worktree>/.cursor/cli.json` (`deny` gana) y por hooks `PreToolUse` de `~/.claude/settings.json`; Cursor frena y Claude lo reanuda con `--resume`. Acciones con permiso: brokers u órdenes, instalar o descargar, borrar, credenciales o config de agentes, commit/push/merge. Dentro de Cursor, solo `Mcp(mmorch:mmorch_check)`. Una vuelta de corrección. El despachador lanza `cursor-agent` sin `SHELL`, `BASH`, `BASH_ENV` ni `MSYSTEM`.
- Pendiente de verificar: cómo interactúa `--force` con `permissions.deny` en modo `-p`.

## Answer

Resuelto por grilling con el usuario (2026-10-01). Captura: [brainstorms/2026-10-01-protocolo-cursor.md](../../../brainstorms/2026-10-01-protocolo-cursor.md).

- Asignar y despertar: llamada directa. Claude corre un comando de mmorch en segundo plano; el comando copia los cambios sin commitear a un worktree, escribe el pedido, lanza `cursor-agent` (sin `SHELL`, con `grok-4.7-medium`) y deja una línea en el canal. Claude Code avisa al terminar.
- Devolver: resumen, acciones bloqueadas, archivos tocados y diff.
- Verificar e integrar: Claude corre los tests y lee el diff en el worktree; si aprueba, `git apply` en la sesión y borra el worktree. Una vuelta de corrección con `--resume`.
- Mientras tanto Claude no toca los archivos asignados a Cursor; si el diff no aplica, integra a mano o descarta.
- Permisos: terminal por `<worktree>/.cursor/cli.json` (`deny` funciona con `--force`, verificado); archivos por hooks `PreToolUse` (el `deny` de escritura no funciona, verificado). Dentro de Cursor, solo `mmorch_check`.
- Fallas técnicas: un reintento, después `gpt-5.4-mini-medium`; tope de 20 minutos; no consume la vuelta de corrección.
- Registro: `logs/cursor_runs.jsonl` con modelo, versión del CLI, tiempo, tokens, aprobación y motivo.
- Pendiente para la implementación: verificar que un hook que bloquea de verdad frena una escritura de Cursor; nombre del comando (propuesta `python -m mmorch.cursor_worker` + herramienta MCP).

## Comments

- 2026-10-04, implementacion (bd `orchestration-ayz`, `mmorch/cursor_worker.py`): sondas reales con cursor-agent 2026.10.01 cambiaron dos puntos de este protocolo. (1) Los hooks del workspace no corren sin interfaz, asi que los archivos no se protegen con hooks: se protegen con el worktree, una huella de lo sensible antes y despues, y permisos al aplicar. (2) El despachador corre SIN `--force`: asi la terminal solo ejecuta la lista permitida; con `--force`, la edicion y la terminal escribian fuera del worktree. La edicion de Cursor igual puede escribir fuera; por eso la huella deja la corrida en ALERTA y `aplicar` se niega.
