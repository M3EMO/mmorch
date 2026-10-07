# Empaquetado: skill, tool MCP o hook
Type: grilling
Status: resolved
Blocked by: 06, 07
Map: ../map.md

## Question

¿Cómo se empaqueta lo que sobreviva a las mediciones, para usarlo en cualquier repo?

Opciones: skill `data-coupling-audit` (método y checklist), tool MCP `mmorch_impact`, hook PreToolUse que adjunta el corte antes de editar, o scaffold de `/new-project` con registro tipado y dueño único desde el día uno. Solo entra lo que tenga número medido.

## Answer

Resuelto por el grilling del ticket 03 (2026-09-25): hook `PreToolUse` en Claude Code y `postToolUse` en Cursor, ambos finos, sobre `mmorch/impacto.py`. Sin tool MCP (el disparo manual se descartó en Q2) y sin skill. El scaffold de `/new-project` con registro tipado y dueño único queda sin decidir.
