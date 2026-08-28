# mmorch en Cursor (perfil `core`)

El MCP server expone 46 tools en perfil `full`; el techo practico de Cursor es
~40 tools por server. El perfil `core` (env `MMORCH_MCP_PROFILE=core`) registra
37 tools curadas y deja fuera 9 nicho/experimentales/Claude-Code-only. El perfil
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
- `MMORCH_MCP_PROFILE=full` (o ausente) expone las 46; en Cursor eso supera el
  techo y Cursor puede truncar/ignorar tools arbitrariamente — usar `core`.

## Que incluye `core` (37) y que queda fuera (9)

Dentro: los patrones core (`mmorch_fan_out`, `mmorch_cascade`, `mmorch_route`,
`mmorch_classify`, `mmorch_check`, `mmorch_tournament`, `mmorch_bucket_rank`,
`mmorch_perfect`, `mmorch_speedup`, `mmorch_review_code`), verificacion
(`mmorch_adversarial_verify`, `mmorch_ensemble_verify`), memoria/recall
(`mmorch_recall`, `mmorch_remember`, `mmorch_learn`, `mmorch_reinforce`,
`mmorch_consolidate`, `mmorch_memory_stats`, `mmorch_intuition`,
`mmorch_flag_contradiction`, `mmorch_vault_write`), spec/planning
(`mmorch_build_spec`, `mmorch_spec_interview`, `mmorch_cynefin`,
`mmorch_rubric_*`), loops/feedback (`mmorch_open_loops`, `mmorch_close_loop`,
`mmorch_record_outcome`, `mmorch_feedback_stats`, `mmorch_pending_review`,
`mmorch_resolve_review`) y budget/health (`mmorch_budget_status`,
`mmorch_error_rates`, `mmorch_metrics_summary`, `mmorch_cache_stats`).

Fuera de `core` (lista exacta: `_NOT_IN_CORE` en `mmorch/mcp_server.py`):

| Tool | Por que fuera |
|---|---|
| `mmorch_ingest_session` | Claude-Code-only (ver limitaciones) |
| `mmorch_session_playbooks` | Claude-Code-only (idem ingest) |
| `mmorch_autoresearch` | job overnight, no interactivo |
| `mmorch_evolve_nightly` | scheduled-task, no interactivo |
| `mmorch_evolve_self` | experimental: auto-evolucion DRY |
| `mmorch_innovate` | experimental: auto-ideacion |
| `mmorch_find_tension` | curiosity, mantenimiento ocasional |
| `mmorch_forget_preview` | gate de mantenimiento, no flujo diario |
| `mmorch_orchestra` | introspeccion del registry, nicho |

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
# core registra <=40 (esperado: 37)
$env:MMORCH_MCP_PROFILE = "core"
C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe -c "from mmorch.mcp_server import mcp; print(len(mcp._tool_manager.list_tools()))"

# full registra todas (esperado: 46)
$env:MMORCH_MCP_PROFILE = "full"
C:/Users/map12/.claude/orchestration/.venv/Scripts/python.exe -c "from mmorch.mcp_server import mcp; print(len(mcp._tool_manager.list_tools()))"
```

En Cursor: Settings → MCP debe listar el server `mmorch` en verde con 37 tools;
probar `mmorch_budget_status` (determinista, no gasta API) como smoke end-to-end.

Test automatizado del contrato: `tests/test_mcp_profile.py`.
