# Protocolo Claude → Cursor (asignar, despertar, devolver, verificar): Brainstorm / Discovery Notes
Date: 2026-10-01 · Goal: resolver el ticket 05 del mapa `.scratch/cursor-trabajador`.

## Summary / key decisions
- Asignar y despertar: llamada directa. Claude corre un comando de mmorch en segundo plano que arma el worktree y lanza Cursor; Claude Code avisa al terminar. Sin despachador por agenda. El comando deja una línea en el canal como registro.
- Permisos: terminal por `cli.json` (`deny` funciona con `--force`); archivos por hooks `PreToolUse` (el `deny` de escritura no funciona).
- Devolver e integrar: el comando copia los cambios sin commitear de la sesión al worktree; Cursor trabaja ahí; el comando devuelve resumen, acciones bloqueadas, archivos y diff; Claude verifica en el worktree (tests + diff); si aprueba, `git apply` en la sesión y borra el worktree. El commit sigue las reglas de siempre (pathspec, push con permiso).
- Mientras Cursor trabaja: Claude sigue, pero no toca los archivos asignados a Cursor (el comando registra la lista al lanzar). Si el diff no aplica, Claude integra a mano o descarta, con el tope de una vuelta.
- Fallas técnicas: si falla el proveedor, un reintento con el mismo modelo y después `gpt-5.4-mini-medium`; tope de 20 minutos por corrida; un error técnico no cuenta como la vuelta de corrección.
- Registro: cada delegación deja una línea en `logs/cursor_runs.jsonl` (modelo, tiempo, tokens, si Claude aprobó y por qué) para medir el ahorro real con el uso.
- Herramientas propias de Cursor: `semSearch` (búsqueda semántica con índice) y `readLints` (diagnósticos del linter) no existen en Claude Code sin IDE. Se mide `semSearch` en el banco de roles (explorador) contra el grep de Claude.

## Contexto de partida
- Ticket 03 (`brainstorms/2026-09-30-donde-entra-cursor.md`): sesiones interactivas primero; tickets completos; cinco acciones con permiso; una vuelta de corrección; solo `mmorch_check`; jerarquía plana.
- Ticket 04: `grok-4.7-medium` 24/24 por defecto; DeepSeek sigue en SDLC; verificar un diff cuesta dos órdenes de magnitud menos que implementar.
- Spike: lanzar `cursor-agent` sin `SHELL`, `BASH`, `BASH_ENV`, `MSYSTEM`.

## Q&A log
### Verificación previa — `--force` contra `permissions.deny` (2026-10-01, corrida mínima con `grok-4.7-medium`)
- `deny: Shell(python)` se respeta con `--force`: el comando quedó bloqueado ("Command blocked by permissions configuration").
- `deny: Write(b.py)` NO se respeta: Cursor creó `b.py` igual.
- Cursor informa en su respuesta final qué quedó bloqueado; Claude puede leerlo y decidir.
- Consecuencia: los permisos de terminal van por `cli.json`; los de archivos (`.env`, credenciales, configuración de agentes) van por hooks `PreToolUse`, que Cursor también ejecuta.
- Evidencia: `orch-spike/scripts/cursor_spike/deny_diag.jsonl`.

### Q1 — Cómo asigna Claude y cómo despierta a Cursor
- Asked: A = llamada directa en segundo plano con aviso de Claude Code; B = issue de bd asignado a Cursor y despachador por agenda. Recomendé A.
- Captured: el usuario confirmó la opción A. No hace falta proceso vigilando bd; el comando registra una línea en el canal.
- Flags: ninguno.

### Q2 — Cómo devuelve Cursor y cómo integra Claude
- Asked: el worktree nuevo parte del último commit y no ve los cambios sin commitear. Recomendé: copiar esos cambios al worktree, devolver resumen + bloqueos + archivos + diff, verificar en el worktree y aplicar con `git apply`.
- Captured: el usuario confirmó el flujo de cuatro pasos.
- Flags: si el `git apply` choca con cambios que Claude hizo mientras Cursor trabajaba, hace falta una regla -> próxima pregunta o implementación.

### Q3 — Qué hace Claude mientras Cursor trabaja
- Asked: riesgo de que el diff no aplique si los dos editan los mismos archivos. Recomendé: Claude sigue, pero no toca los archivos asignados; si igual choca, integra a mano o descarta.
- Captured: el usuario confirmó.
- Flags: ninguno (resuelve el flag del conflicto de la Q2).

### Q4 — Fallas técnicas de Cursor
- Asked: errores del proveedor (3 de 216 en el banco) y corridas colgadas.
- Captured: el usuario confirmó: un reintento con el mismo modelo; después `gpt-5.4-mini-medium`; tope de 20 minutos con corte y aviso; el error técnico no consume la vuelta de corrección.
- Flags: ninguno.

### Pregunta lateral del usuario (2026-10-01)
- "Hay herramientas propias de cursor que sean mejor que las de claude? para programar?" -> se investiga y se responde fuera del árbol del protocolo.

### Q5 — Registro de cada delegación y herramientas propias de Cursor
- Contexto: el paquete del CLI (versión 2026.09.28, actualizada sola desde 2026.08.25) trae `semSearchToolCall` y `readLintsToolCall`, además de las herramientas comunes (read, edit, grep, glob, shell, task, plan, web, MCP).
- Asked: (a) medir `semSearch` en el banco de roles contra el grep de Claude; (b) registrar cada delegación en `logs/cursor_runs.jsonl`.
- Captured: el usuario confirmó las dos.
- Flags: el CLI se actualiza solo; registrar la versión en cada corrida para que los resultados sean comparables.

### Cobertura y cierre
- Tema de implementación anotado, sin decidir (queda fuera del destino del mapa): nombre y lugar del comando, propuesta `python -m mmorch.cursor_worker` más una herramienta MCP que lo llame.
- El usuario cerró el ticket ("dale").

## Open flags (pending input)
- El CLI se actualiza solo -> guardar la versión de `cursor-agent` en cada línea de `logs/cursor_runs.jsonl` y en los bancos.
- Verificar que un hook `PreToolUse` que bloquea (no uno roto) frena una escritura de Cursor -> al implementar.
