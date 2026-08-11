# CLAUDE.md describe un server que ya no existe (drift del contrato de integración)

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: S (solo doc)
Eje: coherencia
Evidence: orchestration/CLAUDE.md:9-20,44 · mcp_server.py · mmorch/config.py:161 · pyproject.toml

CLAUDE.md dice "20 tools" y lista 25 vs 46 reales en `mcp_server.py` (21 sin documentar);
"tag v1.1" vs pyproject 1.2.0; verificador "gemini-2.5-flash" vs
`DEFAULT_VERIFIER="gemini-3.1-flash-lite"`.

**Fix:** sección de tools como puntero a `mcp_server.py` (no lista literal); versión y
modelos citando `config.py` como fuente única.
