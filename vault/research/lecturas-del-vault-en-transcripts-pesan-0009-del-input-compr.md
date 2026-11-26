---
title: "Lecturas del vault en transcripts: pesan 0,009% del input, comprimir notas ahorra disco"
created: 2026-11-26
tags: [research, orchestration, vault-y-compresor, vault, medicion]
status: seed
---
Medición sobre transcripts desde 2026-10-26 (3764 archivos jsonl, subagentes incluidos). Script: scratchpad vault_reads.py.

Medido:
- Lecturas del vault: 53 (Read 6, Bash cat/sed/head 25, Bash grep 22). mmorch_recall: 0 llamadas contadas.
- Escrituras: 67 mmorch_vault_write, 23 Edit y 2 Write. El resumen previo decía ~58; no coincide.
- Notas: 258. Tamaño p50 ~1.5k tokens, p90 ~4.5k, max ~42k (estimado, bytes/3.5).
- Input del mes: 238,6M tokens de cache_creation y 5,52G de cache_read, 84k de input directo (usage real).

Estimado (chars/3.5 del tool_result, no hay contador real):
- Tokens por lecturas del vault: ~21k en el mes.
- Fracción: ~0,009% del input sin cache_read; ~0,0004% con cache_read.
- Fracción media leída por nota: ~0,43 (solo 5 lecturas completas; el resto son tramos o grep).

Límite: 25 de 53 lecturas no se asignaron a una nota (globs o regex en el comando).

Conclusión: comprimir notas del vault ahorra disco, no cupo real. Las lecturas son marginales frente al input total.
