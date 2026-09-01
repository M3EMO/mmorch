# mmorch en Cursor (perfil `core`)

El MCP server expone el perfil `core` (15 tools) **por default** desde la poda
2026-08-30; `MMORCH_MCP_PROFILE=full` expone el catalogo completo (conteo vivo:
`docs/generated/catalog.md`, que inventaria el codigo, no lo que se registra).
El techo practico de Cursor es ~40 tools por server. El perfil
se lee a import-time: cambiarlo requiere reiniciar el server (Cursor lo relanza
al tocar `mcp.json`).

## Configuracion `~/.cursor/mcp.json`

Requiere el paquete instalado (`pip install -e .` en el venv del repo, W2.1);
el entry point `mmorch-mcp` vive en `.venv/Scripts/`.

```json
{
  "mcpServers": {
    "mmorch": {
      "command": "C:/Users/map12/.claude/orchestration/.venv/Scripts/mmorch-mcp.exe",
      "env": {
        "MMORCH_MCP_PROFILE": "core",
        "MMORCH_HOME": "C:/Users/map12/.claude/orchestration"
      }
    }
  }
}
```

Alternativa sin entry point (mismo efecto):

```json
"command": "C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe",
"args": ["-m", "mmorch.mcp_server"]
```

Notas:

- `MMORCH_HOME` apunta la raiz de ESTADO (logs, bandits, memoria, `.env` con
  las API keys). Con el checkout como home, Cursor y Claude Code comparten la
  misma memoria/bandits. Para una instancia aislada, apuntarlo a otro dir y
  copiar ahi el `.env`.
- El `.env` se carga desde `MMORCH_HOME/.env` (no depende del cwd del
  workspace de Cursor — providers.py lo resuelve via paths.home()).
- `MMORCH_MCP_PROFILE=full` expone las 47; en Cursor eso supera el techo y Cursor
  puede truncar/ignorar tools. Ausente o vacio = `core`, que es lo que se quiere:
  el bloque `"MMORCH_MCP_PROFILE": "core"` de arriba quedo redundante pero
  explicito, y no molesta.

## Que incluye `core` (15 tools) y que queda fuera

**`core` es ahora el default** (poda 2026-08-30) y sale de la telemetria, no de
criterio a ojo. `logs/mcp_calls.jsonl`, 53 dias (2026-07-08 → 08-30, 269
llamadas): **11 de 47 tools se invocaron alguna vez**, y 5 concentran el 97% —
`budget_status` (136), `record_outcome` (52), `review_code` (39),
`adversarial_verify` (27), `vault_write` (7).

Dentro (11 con uso medido): `budget_status`, `record_outcome`, `review_code`,
`adversarial_verify`, `ensemble_verify`, `vault_write`, `recall`, `remember`,
`check`, `cynefin`, `innovate`.

Dentro sin llamadas, por excepcion escrita: `canal` (nacio 2026-08-30, no tuvo
ventana) y `build_spec` / `route` / `spec_interview` (los nombra
`~/.claude/skills/perfect/SKILL.md`).

Fuera: las **32 que no se llamaron nunca** en la ventana. No son nuevas —
`fan_out`, `tournament`, `cascade` y `classify` son del 2026-06-07, el dia
fundacional. Siguen implementadas y testeadas; `MMORCH_MCP_PROFILE=full` las
registra todas. Esto recorta **superficie de decision**, no capacidad: cada tool
que el orquestador no usa igual la lee antes de elegir, en cada turno.

Volver a exponer una = sacarla de `_NOT_IN_CORE` y reiniciar el server. Si el
flujo con Cursor empieza a usar `fan_out`/`cascade` de verdad, la telemetria lo
va a mostrar y se revierte con evidencia.

La lista exacta de las 32 vive en `_NOT_IN_CORE` (`mmorch/mcp_server.py`) — no se
duplica aca a proposito: una tabla de 32 filas en un doc driftaria contra el
codigo, y `tests/test_mcp_profile.py` ya congela el set contra la telemetria.

Dos de las 32 estan fuera por una razon que NO es la telemetria y sobrevive a
cualquier medicion: `mmorch_ingest_session` y `mmorch_session_playbooks` son
Claude-Code-only (ver limitaciones abajo) — en Cursor no funcionarian igual.

## Limitaciones conocidas en Cursor

- **Ingest de sesiones es Claude-Code-only.** La matriz canonica
  (`docs/production-readiness/00-canonical-matrix.md`) lo marca fragil:
  "sessions.py:183 — depende del formato JSONL de `~/.claude/projects`
  (Claude Code only; ciego en Cursor/standalone)". El flywheel de playbooks no
  se alimenta desde sesiones de Cursor; el resto de la memoria
  (remember/recall/learn) funciona igual.
- Hooks y skills de Claude Code (`workflow-suggester`, `/verify-cross`, etc.)
  no viajan: en Cursor solo estan las tools MCP.
- El perfil es por proceso: dos clientes con perfiles distintos son dos
  servers (misma memoria si comparten `MMORCH_HOME`).

## Smoke de verificacion

```powershell
# core <=40; full = catalogo generado
$env:MMORCH_MCP_PROFILE = "core"
C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe -c "from mmorch.mcp_server import mcp; print(len(mcp._tool_manager.list_tools()))"

$env:MMORCH_MCP_PROFILE = "full"
C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe -c "from mmorch.mcp_server import mcp; print(len(mcp._tool_manager.list_tools()))"
```

En Cursor: Settings → MCP debe listar el server `mmorch` en verde (perfil `core`);
probar `mmorch_budget_status` (determinista, no gasta API) como smoke end-to-end.

Test automatizado del contrato: `tests/test_mcp_profile.py`.
