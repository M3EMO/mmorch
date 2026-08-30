---
title: auditoria mmorch/babel.py 2026-08-23
status: seed
tags: [mmorch, self-audit]
created: 2026-08-23
---

El módulo babel.py implementa compresión de documentos con gates de calidad, pero presenta problemas estructurales de acoplamiento oculto con el vault, duplicación de lógica de chunking, y un bug en el manejo de chunks que puede producir salidas incompletas.

## Findings (sobrevivieron refutacion 5/5 — 2 estructurales, 1 bugs, 1 de principios)

- **Acoplamiento oculto: ingest() escribe directamente en VAULT global sin inyección** [alta/estructural]: ingest() usa VAULT importado de .vault como destino fijo. El self-check parchea globals()['VAULT'] para evitar tocar el vault real, pero esto es frágil: cualquier otro módulo que importe VAULT directamente (no vía globals) rompería el aislamiento. La regla ADR-0001 (estado en memoria, no releer disco) se viola al escribir directamente sin pasar por una capa de servicio que gestione el estado.
- **Duplicación de lógica de chunking entre encode() y _chunks()** [media/estructural]: encode() decide si chunkear basándose en len(text) > CHUNK_CHARS, y luego _chunks() vuelve a verificar si hay más de un chunk. La lógica de decisión está duplicada: si _chunks() devuelve un solo chunk (porque el texto es indivisible), encode() lo procesa directo, pero la condición de entrada ya garantizaba que era grande. Esto crea dos fuentes de verdad sobre cuándo chunkear.
- **Chunking pierde el último chunk si el texto termina exactamente en el límite** [media/bug]: En _chunks(), si el último párrafo completa exactamente el límite (len(cur) + len(p) == limit), se agrega a 'out' pero 'cur' se resetea a p, y al final del loop 'cur' contiene el último párrafo que ya fue agregado, resultando en duplicación o pérdida. El código debería verificar si cur ya fue agregado antes del append final.
- **Violación del principio de inyección de dependencias en fidelity()** [media/principio]: fidelity() acepta call_fn inyectado, pero internamente usa _lexicon_text() que lee del disco en cada llamada. El lexicon debería ser inyectable como parámetro (lexicon_text=) para permitir tests sin disco y cumplir el principio de seams + injection. Actualmente el self-check no puede probar el flujo con lexicon simulado.
- **Manejo inconsistente de errores en fidelity(): JSON inválido del questioner no se distingue de fidelidad baja real** [baja/otro]: Cuando el questioner devuelve JSON inválido, fidelity() retorna score=0.0 con n_questions=0. Esto es conservador pero confunde el diagnóstico: ingest() reportará 'fidelity 0.0 < 0.8' sin indicar que el problema fue el questioner, no la calidad del babel. Sería más claro retornar un flag 'questioner_error' separado del score.
