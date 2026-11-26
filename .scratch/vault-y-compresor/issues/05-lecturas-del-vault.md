# Lecturas y tamaños de las notas del vault
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Medí cómo se usa el vault en los transcripts del último mes. Contá qué notas se leen, con qué tool y cuántas veces. Medí la distribución de tamaño en tokens de las notas y del tramo leído. El resultado dice si comprimir notas ahorra cupo real o solo disco.

## Answer

Medido (transcripts desde 2026-10-26, 3764 archivos, subagentes incluidos):
- 53 lecturas del vault: Read 6, Bash cat/sed/head 25, Bash grep 22. `mmorch_recall`: 0.
- Escrituras: 67 `mmorch_vault_write`, 23 Edit, 2 Write. El resumen previo (~58 escrituras, ~50 lecturas) acierta en lecturas y no en escrituras.
- Notas: 258. Tamaño estimado: p50 ~1.5k tokens, p90 ~4.5k, max ~42k.
- Input del mes (usage real): 238,6M cache_creation, 5,52G cache_read, 84k input directo.

Estimado (chars/3.5 del tool_result):
- Lecturas del vault: ~21k tokens en el mes, o sea ~0,009% del input sin cache_read.
- Fracción media leída por nota: ~0,43. Solo 5 lecturas son completas.
- 25 de 53 lecturas no se asignaron a una nota (globs o regex).

Conclusión: comprimir notas del vault ahorra disco, no cupo real.

Nota en el vault: "Lecturas del vault en transcripts: pesan 0,009% del input, comprimir notas ahorra disco" (vault/research/lecturas-del-vault-en-transcripts-pesan-0009-del-input-compr.md).
