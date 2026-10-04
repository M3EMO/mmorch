---
title: "Contener a cursor-agent sin interfaz en Windows: que frena y que no"
created: 2026-10-04
tags: [research, orchestration, cursor, seguridad, permisos, windows, despachador]
status: measured
confidence: "alta: sondas reales con cursor-agent 2026.10.01, una corrida por caso"
sources: [bd orchestration-ayz, mmorch/cursor_worker.py (d7634d4), .scratch/cursor-trabajador/issues/05-protocolo-asignar-despertar-devolver.md]
---
## Pregunta

¿Que mecanismos frenan de verdad a `cursor-agent -p` (sin interfaz) en Windows cuando trabaja como empleado de Claude en un worktree descartable?

## Medido (2026-10-04, cursor-agent 2026.10.01, `grok-4.7-medium`)

| Mecanismo | Resultado |
|---|---|
| Hook `PreToolUse` en `<ws>/.claude/settings.json` | no corre |
| Hook `preToolUse` en `<ws>/.cursor/hooks.json` | no corre, ni con el comando sin comillas |
| `--sandbox enabled` | error: "Sandbox requires macOS or Linux" |
| Con `--force` | la edicion y la terminal escriben fuera del workspace |
| Sin `--force` y `allow: [Shell(cmd*)]` | la terminal rechaza lo que no esta en la lista (verificado con `Set-Content` fuera) |
| `Write(<ruta absoluta>)` o `Write(../**)` en `deny` | no coincide: la edicion fuera del workspace pasa |
| `Write(**)` en `deny` | bloquea todo, tambien adentro |
| `Mcp(*:*)` en `deny` | bloquea las herramientas MCP (Gmail, Calendar y Drive de los plugins de Cursor) |

El CLI si declara las rutas de hooks del proyecto (`.cursor/hooks.json`, `.claude/settings.json`, `.claude/settings.local.json`), pero en estas sondas ninguno corrio.

## Diseño que sale de esto (`mmorch/cursor_worker.py`)

- Cursor corre sin `--force`, con la terminal limitada a tests y git de lectura, y sin MCP.
- La herramienta de edicion no se puede contener: el despachador toma una huella de lo sensible antes y despues de cada vuelta. Cubre config de Claude y Cursor, perfiles de shell, `.ssh`, hooks de git, otras corridas y el `node_modules` enlazado. Si algo cambia, la corrida queda en ALERTA para siempre y `aplicar` se niega.
- Al repo solo vuelve el diff del worktree, y solo con `aplicar`. Borrados y archivos protegidos (tambien por rename y en cualquier nivel) piden permiso.

## Trampas encontradas en la revision adversarial

- `git worktree remove --force` sigue un junction y vacia el destino: hay que quitar los junctions antes.
- `git diff --name-status` detecta renames por default: un rename escondia un borrado y un archivo protegido.
- La configuracion de diff del usuario (`diff.noprefix`, `diff.external`) puede romper el patch: usar `--no-ext-diff --src-prefix=a/ --dst-prefix=b/`.
- Un repo prohibido se detecta tambien por su repo principal y su remoto: un worktree de Portfolio en `%TEMP%` pasaba el filtro por carpeta.

## Riesgo que queda

La edicion de Cursor puede escribir en cualquier archivo que no este en la huella. El aislamiento real necesitaria otro usuario de Windows.
