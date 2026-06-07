# mmorch vault — memoria + corpus de research

Vault Obsidian que sirve de **memoria de largo plazo** de mmorch y **corpus de research**
humano. mmorch lee/escribe notas acá vía `mmorch/vault.py`. Vos navegás con backlinks/grafo.

## Estructura
- `research/` — hallazgos de research (grounded: fuente real + verificado cross-family).
  Cada nota: frontmatter (status, confidence, verifier, sources) + cuerpo + `## Aplicable a mmorch`.
- `memory/` — memoria semántica de mmorch: hechos verificados, decisiones, lecciones. Escribible por mmorch.
- `roadmaps/` — audits + innovation roadmaps (link a `../AUDIT_*.md`, `../INNOVATION_ROADMAP_*.md`).
- `templates/` — plantillas de nota.

## Convenciones
- Wikilinks `[[nota]]` para conectar. Tags `#research #autolearning #mmorch`.
- Toda afirmación con `confidence` y, si grounded, `sources:` reales (no citas inventadas).
- Status: `seed` (idea) → `verified` (cross-family pasó) → `applied` (entró a mmorch) → `refuted`.

## Mapa de contenido (MOC)
- Research autolearning/self-improving AI → ver `research/`
- Decisiones de diseño mmorch → `memory/`
- Auditorías + roadmaps → `roadmaps/`

> mmorch NO navega web. Research grounded = Claude (WebSearch) junta fuentes → mmorch
> (cross-family) refuta/sintetiza → se vuelca acá. Síntesis sin fuente = `status: seed`.
