# Transferencia de los contratos a otra superficie de mmorch
Type: prototype
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿Las cinco categorías de contrato (ventana, precisión y unidades, segmentos, cache, escala) detectan fallas en otra superficie sin escribir tests a medida del mutante?

Superficie candidata: `canal.jsonl` o `workflow.db` (elegir con el inventario del ticket 02). Protocolo: el mismo del banco de acople. Contratos escritos solo desde las categorías, ANTES de generar mutantes; mutantes de dos familias; juez de equivalencia v2; criterio de cierre real >= 70%.

Depende también del resultado de la ronda 4 del banco de acople: si los contratos no generalizan en metrics.jsonl, este ticket se replantea.

## Answer

Cerrado sin hacer (2026-09-27, decisión del usuario). Las 4 rondas del banco de acople mostraron que los contratos por categoría no convergen: cierran 40-70% de las fallas reales nuevas y cada ronda encuentra una propiedad nueva. El valor medido del mapa vino del informe de impacto (tickets 09, 11, 07) y de cambios estructurales (tipo compartido, dueño único, borrar `window_s`), no de enumerar contratos. Los 19 contratos quedaron en `tests/test_metrics_contracts.py` (commit 0b1c4a4) para metrics.jsonl, sin transferirlos a otras superficies.
