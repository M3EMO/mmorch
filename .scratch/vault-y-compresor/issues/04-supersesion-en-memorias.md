# Supersesión en sistemas de memoria de agentes
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Investigá cómo los sistemas de memoria de agentes evitan que un hecho viejo gane a uno nuevo. Cubrí StaleBench, memorias bi-temporales como Zep o Graphiti, mem0 y el chequeo de contradicción al escribir. Anotá qué mecanismo tiene una medición y cuánto cuesta por escritura.

## Answer

| Mecanismo | Medición | Costo por escritura | Tipo |
|---|---|---|---|
| Coseno sin más (base) | StaleBench: 53% de veces devuelve versión vieja | 0 | determinista |
| Reordenar por fecha | current@1 0.925, pero el viejo sigue en contexto (lector -22.9 pts) | 0 | determinista |
| Clave simétrica (trigramas, escribir y leer) | current@1 0.831 sin ancla léxica (coseno+sobrescribir: 0.513) | 0 llamadas LLM | determinista |
| Ledger bi-temporal con regla (sujeto, relación, objeto) y `superseded_by` | MemStrata: error de hecho viejo 15-40% a ~0%; preprint de 1 autor, 7B | 0 llamadas LLM; lectura ~2.1 s | determinista (prosa libre cae a LLM) |
| Zep/Graphiti: LLM compara aristas nuevas con viejas | LongMemEval 71.2% vs 60.2%; knowledge-update +6.5% / -3.4% | 1+ llamada LLM (conteo no publicado) | LLM sobre campos de fecha |
| mem0 ADD/UPDATE/DELETE/NOOP | LOCOMO J 66.9, p95 1.44 s; sin medida aislada de supersesión | 1 extracción + 1 decisión LLM por hecho (aprox.) | LLM |
| Letta/MemGPT, A-MEM | sin medición de supersesión encontrada | llamadas LLM, sin cifra | LLM |

Confianza: alta en StaleBench, Zep y mem0 (leídos del paper); media en MemStrata; baja en Letta, A-MEM y todos los costos por escritura.

Opciones para ~58 notas al mes:
1. Frontmatter `valid_from`, `valid_to`, `superseded_by`; el recall filtra las notas reemplazadas. Cero LLM.
2. Opción 1 más `claim_key`: al escribir, si hay una nota activa con la misma clave, el tool la cierra o avisa. Determinista.
3. Opción 2 más una llamada LLM solo si hay candidatas muy parecidas sin clave. Sin medición de que supere a la 2.

Nota en el vault: "Supersesión en memorias de agentes: qué mecanismo evita que un hecho viejo gane" (`vault/research/supersesión-en-memorias-de-agentes-qué-mecanismo-evita-que-u.md`).
