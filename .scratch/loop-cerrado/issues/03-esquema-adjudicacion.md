# 03 — Esquema de adjudicación nota→proyecto

Type: grilling
Status: resolved
Blocked by: 01

## Question

Diseño del cruce vault→proyectos (universo = registry mmorch, enriquecido con
codegraph donde haya índice):

- Dónde viven los edges nota→proyecto: ¿frontmatter de la nota, sidecar JSON,
  o SQLite de mmorch? (criterio: quién los lee después y con qué frecuencia)
- Scoring: qué define "match fuerte" (umbral), y cómo cita archivo/módulo
  cuando codegraph está disponible.
- Cadencia y costo: nightly con DeepSeek/Gemini; dedup contra adjudicaciones ya
  hechas; qué pasa con notas nuevas vs re-scan de viejas.
- Verificador cross-family refutando matches (invariante mmorch) — ¿siempre o
  solo sobre el top-K?

## Answer

Grilling 2026-08-12.

- **Storage: ambos** — `logs/adjudications.json` (índice keyed por proyecto,
  fuente de verdad para el hook de propuesta, write atómico iohelpers) +
  frontmatter espejo `applies_to: [...]` en cada nota, escrito por el mismo
  nightly (visible en Obsidian). El JSON manda ante divergencia.
- **Match fuerte: LLM juzga + verificador refuta** — DeepSeek propone matches
  con score 0-1 y justificación; Gemini refuta (invariante cross-family).
  Sobrevive con score ≥0.7 = fuerte (habilita propuesta en sesión). Sin
  pre-filtro de embeddings: universo chico (13 notas × 9 proyectos), juzgar
  todo cuesta centavos.
- **Cadencia: nightly incremental** — solo pares nuevos (nota nueva × todos
  los proyectos, proyecto nuevo × todas las notas). Par juzgado no se re-juzga
  salvo cambio de la nota (hash de contenido). Re-scan completo solo manual.
- **Codegraph como enriquecedor**: si el proyecto tiene índice, juez Y
  refutador reciben el mapa de módulos/archivos y el match cita
  archivo/módulo concreto; sin índice, match a nivel proyecto.
