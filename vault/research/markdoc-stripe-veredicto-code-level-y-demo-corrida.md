---
applies_to:
- Portfolio financiero
- orchestration
confidence: alta — clonado, API leída, demo ejecutada con casos válido e inválido
created: 2026-08-12
sources:
- https://github.com/markdoc/markdoc
- npm @markdoc/markdoc 0.5.9
status: evergreen
tags:
- research
- mmorch
- markdoc
- markdown
- validacion
- templates
- reports
- steal-pattern
title: Markdoc (Stripe) — veredicto code-level y demo corrida
---
# Qué es

Framework de autoría de Stripe (motor de stripe.com/docs): Markdown estándar +
tags custom `{% tag attr=val %}` con **schema tipado y validable**, variables
`{% $var %}`, condicionales `{% if %}`, partials, funciones. Pipeline:
`parse() → validate(config) → transform(config) → renderers.html|react`.
Cero dependencias runtime pesadas (markdown-it adentro), ~0 config.

# Estructura del repo (clonado, v0.5.9)

- `src/parser.ts` + `src/tokenizer/` — markdown-it extendido con la gramática `{% %}`
- `src/validator.ts` — el gate: valida tags/atributos/tipos contra el schema del config
- `src/transformer.ts` — AST → árbol renderizable
- `src/renderers/` — html y react
- `src/tags/` — built-ins: conditional (if/else), partial, slot, table
- `src/functions/` — equals/and/or/not/default/debug
- `src/schema-types/`, `src/ast/` — tipos y nodos

# Demo corrida (caso portfolio semanal)

Tag `{% alerta nivel="duro" ticker="EWZ" diff_pp=24.0 %}` con schema
(`nivel` enum ok|blando|duro, `ticker` required). Resultado:

- Doc válido → 0 errores, HTML limpio con clases/data-attrs.
- Doc inválido (lo que emitiría un LLM alucinando) → validate() devuelve:
  enum violado ("Got 'catastrofico'"), atributo faltante ("Missing required
  attribute: 'ticker'") y tag inexistente ("Undefined tag: 'alertta'",
  level critical) — **con número de línea**. Es exactamente un schema-gate
  determinista, mismo espíritu que `gated_json` de mmorch pero para documentos.

# Veredicto (robar patrón, adopción selectiva)

- **SÍ sirve para**: outputs md de modelos baratos que hoy salen free-form —
  reporte semanal del portfolio, informes de vuelco, reportes de auditoría.
  El modelo emite md+tags, `validate()` frena alucinaciones de estructura con
  feedback line-exact (re-ask barato), y el mismo doc renderiza a HTML para
  dashboard/mail. Complementa flint (flint=charts, markdoc=documento).
- **NO sirve para**: el vault/notas (frontmatter+md plano ya alcanza, sumar
  sintaxis `{% %}` a Obsidian rompe compatibilidad), ni CLAUDE.md/docs de repos.
- **Riesgo bajo**: MIT, Stripe lo usa en producción, API chica y estable.

# Integración candidata (si se decide)

`gated_markdoc(model, prompt, schema_tags)` en mmorch espejo de `gated_json`:
gen → Markdoc.validate → errores como feedback → retry → SchemaGateError.
Render final = paso determinista aparte. Esperar a tener UN reporte recurrente
que lo pida de verdad (candidato: revision_semanal cuando se reconcilie el
portfolio post-reestructuración).
