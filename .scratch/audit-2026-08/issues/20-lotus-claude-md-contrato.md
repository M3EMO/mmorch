# Lotus sin CLAUDE.md: el contrato de consumo mmorch vive solo en api.js

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: S
Eje: coherencia
Evidence: Lotus/src/lib/api.js:1-30 · mmorch/server.py:755-792

Lotus (único consumidor HTTP de mmorch) sin CLAUDE.md/AGENTS.md; el conocimiento del
backend (URL, token X-Token, fallback mock) vive solo en código.

**Fix:** CLAUDE.md mínimo según convención new-project con sección de consumo mmorch +
puntero a `mmorch/server.py:755-792` como lista canónica de endpoints.
