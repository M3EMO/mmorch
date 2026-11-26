---
title: "Supersesión en memorias de agentes: qué mecanismo evita que un hecho viejo gane"
created: 2026-11-26
tags: [research, orchestration, vault-y-compresor, supersesion, memoria]
status: seed
confidence: medium
sources: ["https://arxiv.org/abs/2611.19752", "https://arxiv.org/abs/2606.26511", "https://arxiv.org/abs/2501.13956", "https://arxiv.org/abs/2504.19413", "https://arxiv.org/abs/2605.06527", "https://arxiv.org/abs/2610.05732", "https://arxiv.org/abs/2502.12110", "https://lacuna.tiptreesystems.com/work/stalebench-isolating-stale-retrieval-in-agent-memory"]
---
## Problema
Un recall por similitud devuelve el hecho viejo y el nuevo con puntaje casi igual. Un sistema necesita una regla que marque cuál vale hoy.

## Mecanismos y mediciones
- **StaleBench** (arXiv 2611.19752, Lacuna). Es un benchmark, no un mecanismo. Con 892 hechos reales que cambian, el coseno devuelve una versión vieja 53% de las veces.
- Reordenar por fecha sube current@1 a 0.925. Pero el hecho viejo sigue en el contexto: un lector pierde 22.9 puntos. Contamination@5 se queda en 0.974.
- Keying sin ancla léxica: umbral de coseno con sobrescritura da current@1 0.513. Un enlazador difuso da 0.714. Clave simétrica (mismo matcher de trigramas al escribir y al leer) da 0.831. El oráculo da 0.827.
- Costo: el paper no reporta costo por escritura. Dice que la clave simétrica no usa modelo. El rung "LLM keyer" muestra los 4 candidatos mejores a un modelo.
- **Ledger bi-temporal determinista** (MemStrata, arXiv 2606.26511). Una regla (sujeto, relación, objeto) cierra `valid_to`, escribe `superseded_by` y abre una fila nueva. No usa coseno ni LLM. Un hash de texto normalizado descarta repetidos exactos.
- Medición: RAG sirve el valor viejo 15-40% de las veces; MemStrata ~0%. La precisión en evolución sube de 0.20-0.47 a 0.95-1.00. Latencia de lectura ~2.1 s contra 16-18 s con rerank por LLM. Es un preprint de un solo autor con un modelo de 7B local. Es evidencia débil.
- Límite: solo cubre hechos con forma de triple. La prosa libre cae a similitud más un juez LLM.
- La misma fuente mide que el coseno separa "contradice" de "repite" con AUROC 0.59, casi azar.
- **Zep / Graphiti** (arXiv 2501.13956). Cuatro marcas de tiempo por arista: creada y expirada en el sistema, válida e inválida en el mundo. Al llegar una arista nueva, un LLM la compara con aristas relacionadas y fija `t_invalid` de la vieja. El paper no da llamadas por escritura.
- Medición: LongMemEval con gpt-4o 71.2% contra 60.2% de contexto completo, latencia 2.58 s contra 28.9 s. En la categoría knowledge-update: +6.5% con gpt-4o y -3.4% con gpt-4o-mini. La ganancia general no prueba que la supersesión funcione.
- **mem0** (arXiv 2504.19413). Por cada hecho candidato recupera los top-s memorias parecidas. Una llamada LLM por herramienta elige ADD, UPDATE, DELETE o NOOP. La variante de grafo marca relaciones como inválidas en vez de borrarlas.
- Medición: LOCOMO J 66.88 (Mem0) y 68.44 (Mem0g) contra 72.90 de contexto completo. Latencia total p95 1.44 s. No hay una medición aislada de supersesión. Costo: al menos una llamada LLM de extracción más una de actualización por escritura (GPT-4o-mini); el paper no da el conteo exacto.
- **Letta / MemGPT.** El agente edita sus bloques de memoria con herramientas, es decir con LLM. No encontré medición de supersesión ni costo por escritura. La página de docs consultada no describe el mecanismo. Confianza baja.
- **A-MEM** (arXiv 2502.12110). Al agregar una nota, un LLM genera enlaces y actualiza notas viejas. El resumen no menciona contradicción ni supersesión, ni da costo. Sin medición para este problema.
- **STALE** (arXiv 2605.06527). Benchmark de conflicto implícito: 400 escenarios, 1200 consultas. El mejor modelo llega a 55.2%. Reconocer que algo es viejo no implica aplicar el dato nuevo. El prototipo CUPMem no tiene costo reportado.
- **PACMI** (arXiv 2610.05732). Grafo de procedencia con estados de validez que se propagan a dependientes. 100 casos; errores de 11 a 3 al activar la cascada, sin significancia. Muestra chica.
- **Chequeo de contradicción al escribir.** Es la fase de mem0 y Zep. Cuesta una llamada LLM más una búsqueda vectorial por escritura. ERASE (reimplementado en StaleBench) usa una llamada por llegada: 1395 keep, 88 rewrite, 147 delete en qwen2.5:14b.

## Determinista contra LLM
- Determinista: fechas `valid_from`/`valid_to`, campo `superseded_by`, hash de texto, clave (sujeto, relación), matcher de trigramas, filtro de filas no vigentes al leer.
- LLM: decidir si dos textos hablan del mismo hecho, decidir si se contradicen, extraer el triple de prosa libre.
- Dato transversal: sin la ancla léxica, el umbral de coseno falla por decenas de puntos. El LLM-juez al escribir es el método más caro y el menos medido.

## Opciones para un vault de ~58 notas al mes
1. Campos `valid_from`, `valid_to` y `superseded_by` en el frontmatter, escritos a mano o por el tool de escritura. El recall filtra las notas con `superseded_by`. Costo: cero llamadas LLM.
2. Opción 1 más una clave de tema (`topic`/`claim_key`) en el frontmatter. Si ya existe una nota activa con la misma clave, el tool avisa o cierra la vieja. Determinista; la clave la pone el autor.
3. Opción 2 más una sola llamada LLM al escribir, solo cuando hay candidatas con alta similitud. A 58 notas al mes son pocas llamadas, pero no hay medición de que supere a la opción 2.

## Confianza
- Alta: cifras de StaleBench (53%, 0.925, tabla de keying) y de Zep/mem0 tomadas del texto del paper.
- Media: MemStrata (preprint de un autor, 7B).
- Baja: Letta, A-MEM, costos por escritura (ningún paper los reporta con precisión).
