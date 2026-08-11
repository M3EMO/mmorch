# Entradas de tracker — EJE=coherencia (2026-08-10)

Para mergear por el orquestador en `.scratch/audit-2026-08-10/issues/` según el formato de
`docs/agents/issue-tracker.md` (un archivo por ticket, `NN-<slug>.md`, Status al tope).
Rankeadas: severidad primero, a igual severidad menor esfuerzo primero.

---

## 01-claude-md-drift-contrato.md
Status: open
Type: task
Severidad: IMPORTANTE
Esfuerzo: bajo (solo doc)

`orchestration/CLAUDE.md` describe un server que ya no existe: "20 tools" y lista de 25
(CLAUDE.md:9-18) vs 46 reales en `mcp_server.py` (21 sin documentar); "tag v1.1" (:20) vs
pyproject 1.2.0; verificador "gemini-2.5-flash" (:44) vs `DEFAULT_VERIFIER="gemini-3.1-flash-lite"`
(`mmorch/config.py:161`). Fix: sección de tools como puntero a `mcp_server.py` (no lista literal),
versión y modelos citando `config.py` como fuente única.
Verificado: adversarial_verify SURVIVES ("invalida el contrato de integración").

---

## 02-tracker-doble-fuente-verdad.md
Status: open
Type: task
Severidad: IMPORTANTE
Esfuerzo: bajo (2 ediciones de doc)

Contradicción de contrato en el mismo repo: `CLAUDE.md:146-148` manda bd/beads para issue
tracking durable; `docs/agents/issue-tracker.md:3` declara que los issues viven en `.scratch/`.
Ambos sistemas vivos (`.beads/` con estado + `.scratch/` usado por wayfinder). Fix: línea de
partición explícita en ambos docs (bd = backlog durable; `.scratch/<effort>/` = mapas wayfinder
+ tickets del esfuerzo) + regla de promoción a bd.
Verificado: adversarial_verify SURVIVES ("ambigüedad operativa").

---

## 03-projects-json-higiene.md
Status: open
Type: task
Severidad: NICE-TO-HAVE
Esfuerzo: bajo

`projects.json` contaminado: path pytest temporal muerto ("repo" → AppData\Local\Temp\
pytest-of-map12\...), home dir completo ("map12") y Desktop entero ("Claude") registrados.
Causa: `scripts/autoregister_project.py:13` (_SKIP solo excluye orchestration) en SessionStart;
sin GC (`mmorch/projects.py:57-65` resolve() falla sobre muertas pero nadie poda). Fix: filtro
temp/home en _SKIP + `prune()` dry-run-default en projects.py.
Verificado: refutado como IMPORTANTE (2 rondas), residuo de mantenimiento concedido.

---

## 04-lotus-claude-md-contrato.md
Status: open
Type: task
Severidad: NICE-TO-HAVE
Esfuerzo: bajo

Lotus (único consumidor HTTP de mmorch) sin CLAUDE.md/AGENTS.md/docs-claude; el conocimiento del
backend (URL, token X-Token, fallback mock) vive solo en `Lotus/src/lib/api.js:1-30`. Fix:
CLAUDE.md mínimo según convención new-project con sección de consumo mmorch + puntero a
`mmorch/server.py:755-792` como lista canónica de endpoints.
Verificado: refutado como IMPORTANTE (2 rondas: "deuda de documentación"), entra degradado por
mandato del eje (desviación de convención = hallazgo).
