# Fuentes de verdad (mmorch)

Un tipo de hecho → un dueño. El resto apunta, se genera, o está fechado y no es verdad.
Ratchet: `python -m mmorch.docgen --check`.

| Hecho | Dueño | No copiar en |
|---|---|---|
| Qué módulos/tools existen, conteos | código → `docs/generated/catalog.md` | contratos de agente |
| Modelos, familias, roles, precios de registry | `mmorch/config.py` (+ `prices.json` override) | CLAUDE, AGENTS, GOAL, capabilities |
| Invariantes, non-goals, métricas de éxito | `GOAL.md` | AGENTS/CLAUDE (solo puntero) |
| Ruteo de turno (cupo vs API) | `CLAUDE.md` | capabilities |
| Índice cross-agent | `AGENTS.md` | el texto de GOAL |
| Vocabulario | `CONTEXT.md` | implementaciones |
| Cuándo elegir un patrón | `docs/capabilities.md` | firmas, keys de modelo, conteos |
| Por qué (irreversible) | `docs/adr/` | catálogos |
| Cómo se escribe código | `docs/coding-principles.md` | el modelo del sistema |
| Research, ablaciones, auditorías | `vault/` | `docs/` como si estuvieran al día |
| Snapshots de producto (sólido/frágil a una fecha) | `docs/production-readiness/` | contratos; no re-sincronizar a mano |

## Capas

1. **Mecánica** — el código. Docstring de módulo = **una línea** (la come el catálogo). El *por qué* local es un comentario junto a la decisión, no un ensayo al tope del archivo. Nada de markdown al lado de cada `.py`.
2. **Contrato** — GOAL / CLAUDE / AGENTS / CONTEXT. Cortos, sin listas que el código ya tiene.
3. **Vista** — `docs/generated/`. Se commitea; un humano no la edita.
4. **Juicio** — capabilities (elección), ADRs (trade-off).
5. **Snapshot** — production-readiness y notas de vault. Fecha + “no es SSOT”. Si un hallazgo sigue vivo, va a bd o al código, no se mantiene la matriz.

## Beads / scratch / memoria

Misma regla de partición: un hecho no vive en dos trackers. Detalle: `docs/agents/issue-tracker.md`.
