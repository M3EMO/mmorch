---
title: "Vault y compresor: babel no paga, el arranque sí (2026-11-26)"
created: 2026-11-26
tags: [research, orchestration, research, vault, babel, compresion, memoria, lacuna, alien-space, veredicto]
status: seed
confidence: alta en las mediciones propias; media en cifras de papers leídas por abstract
sources: ["https://lacuna.tiptreesystems.com/work/stalebench-isolating-stale-retrieval-in-agent-memory", "https://lacuna.tiptreesystems.com/work/memory-tax-dissecting-the-net-value-of-agent", "https://lacuna.tiptreesystems.com/work/fundamental-limits-of-prompt-compression-a-rate-distortion", "https://lacuna.tiptreesystems.com/work/beyond-token-savings-a-systematic-study-of-context", "https://lacuna.tiptreesystems.com/work/context-has-a-price-adding-and-removing-context", "https://arxiv.org/abs/2606.19857", "https://arxiv.org/abs/2603.29919", "https://arxiv.org/abs/2403.12968", medición propia 2026-11-26]
---
# Vault y compresor

Fecha: 2026-11-26. Fuentes: mediciones propias sobre transcripts y vault, un subagente con Lacuna y web, y una corrida de Alien.

## Mediciones propias

- El último mes tuvo 58 escrituras al vault y unas 50 lecturas de notas.
- Ninguna sesión leyó un `.babel.md` en el último mes.
- Con el contador de tokens de Anthropic, 17 pares babel dejan 63% de los tokens del original.
- El lexicon cuesta 1102 tokens. Una nota babel más el lexicon cuesta 96% del original.
- Babel corre en cada escritura y gasta API.
- El vault tiene 126 notas `seed` de 242, y ninguna nota marca `superseded_by` ni vencimiento.
- El README del vault nombra una carpeta `memory/` que no existe y docs ya borrados.
- El arranque de una sesión cuesta unos 67k tokens.
- Las descripciones de ECC suman un techo de unos 47k tokens. Claude Code ya muestra muchos skills solo por nombre.

## Papers (medido, leído por abstract o por página de Lacuna)

- StaleBench: el retrieval por coseno devuelve la versión vieja 53% de las veces. Invalidar lo viejo rinde más que reordenar.
- Memory Tax: varios de 11 sistemas de memoria rinden menos que un RAG ingenuo. Resumir esconde evidencia.
- Rate-distortion de compresión: comprimir conociendo la pregunta supera a comprimir sin la pregunta.
- Beyond Token Savings: limpiar resultados viejos de tools da 0.71x de costo. Un resumen completo por LLM baja SWE-bench unos 11 puntos.
- Context Has a Price: con caché, borrar contexto del medio encarece entre 38% y 116%.
- SkillReducer: comprimir en prosa normal ahorra 48% en descripciones y sube la calidad 2.8%.

## Alien (anclas 27, 114 y 88; corpus hasta 2025)

- 27 + 73 + 101: surprisal, perturbaciones y tokenización BPE. La idea es comprimir según el tokenizer real.
- 27 + 45 + 153: surprisal y edición por operaciones locales. La idea es borrar tramos en vez de reescribir.
- 35 + 114 + 144: lost-in-the-middle, descomposición recursiva y selección submodular. La idea es elegir fragmentos según la pregunta.
- Alien recombina ideas conocidas; LLMLingua ya usa surprisal.

## Veredicto

- Babel no paga en el uso real. Conviene apagar su ingest.
- El vault necesita supersesión y chequeo de contradicciones al escribir, no compresión.
- Un compresor rinde más sobre el contexto de arranque o con selección según la pregunta.
