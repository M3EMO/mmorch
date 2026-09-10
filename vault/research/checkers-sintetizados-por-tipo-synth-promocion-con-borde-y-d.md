---
title: Checkers sintetizados por tipo: synth, promocion con borde y decorrelacion por metodo — 2026-09-10
created: 2026-09-10
tags: [research, mmorch, ablation, synth, checkers, decorrelacion, oneflow, evalplus, sdlc]
status: validated
confidence: alta en lo medido; la pregunta de familia queda sin respuesta por falta de errores
sources: [ablation_stages_hard.py, ablation_synth_kinds.py, logs/ablation_results.jsonl, logs/ablation_synth_kinds_items.jsonl, EvalPlus NeurIPS 2023, LiveCodeBench]
---
## Pregunta
Idea del usuario (2026-09-09): que el modelo no calcule, que escriba la funcion y el sandbox la corra. Y despues: que la funcion se sintetice UNA vez por tipo de problema y se cachee (gate `synth:`). Eso es checkers.py escrito por el modelo, con promocion contra verdad computada.

## Resultado 1 — metodo, mismo modelo (ablation_stages_hard, n=50-80, gold set dificil)
- deepseek-chat calculando a mano: especificidad 0.35-0.50, $0.0046/llamada.
- code:deepseek-chat (escribe solve() por item): 0.98, $0.00003/llamada (150x mas barato).
- code:deepseek-v4-pro: 1.000 en 130 items.
- synth:deepseek-chat (una funcion por tipo, cache): 1.000 en 50 items con ~10 llamadas totales.
- phi entre manual y code del MISMO modelo: -0.07 / +0.06 / +0.11 en tres runs. Los errores se decorrelacionan por METODO, no por familia. Mismos pesos.
- Los falsos rechazos residuales de code: fueron formato (expresion pelada, eco del system prompt), no aritmetica.

## Resultado 2 — decorrelacion a nivel TIPO (ablation_synth_kinds, 57 tipos algoritmicos, 6 modelos, $0.77, seed 46)
Diseño: cada tipo con solver de referencia propio (verificado por fuerza bruta); la funcion recibe `params` tipados, no prosa; promocion = 3 items aleatorios + 1 de BORDE (instancia minima legal, idea EvalPlus: los tests de borde bajan pass@1 10-29 puntos); un tipo con limite de tiempo (LiveCodeBench). Falla de tipo = no promovida o falso rechazo o bug perdido en 7 items de test.

| modelo | promovidos | fallados | bugs perdidos | costo |
|---|---|---|---|---|
| deepseek-chat (sin thinking) | 53/57 | 4 | 0 | $0.003 |
| deepseek-v4-pro | 57/57 | 0 | 0 | $0.37 |
| deepseek-reasoner | 57/57 | 0 | 0 | $0.03 |
| glm-4.5-air | 54/54 | 0 | 0 | $0.21 |
| glm-5.2 | 49/49 | 0 | 0 | $0.16 |

Cero bugs perdidos en 274 funciones. Los 4 fallos de deepseek-chat son bugs de spec reales: p*q en vez de lcm en inclusion-exclusion; terminos cambiados en la recurrencia de tiling; "mayor primo" que devuelve 1 cuando el numero se factoriza entero (lo atrapo SOLO el item de borde); matriz de transferencia mal armada en "cadenas sin ab".

**phi indefinido en los 10 pares: no hay errores que correlacionar.** Cuatro de cinco modelos no fallan ningun tipo. La pregunta misma-familia vs cross-familia queda sin respuesta a esta dificultad. No se generaliza a tareas subjetivas (invariante OneFlow de GOAL.md sigue con su alcance).

## Lo que si queda establecido
1. Para tareas bien especificadas, tipadas y de una funcion, un modelo con razonamiento sintetiza un checker correcto casi siempre. deepseek-reasoner lo hace por $0.03 las 57 funciones: 12x mas barato que v4-pro con el mismo resultado. Para el SDLC el sintetizador de chequeos es reasoner.
2. La promocion con borde atrapa bugs que la promocion aleatoria deja pasar (caso "mayor primo").
3. La brecha entre chat y los demas es obediencia de formato y bugs de spec, no aritmetica.

## Lecciones de arnes (pagadas 4 veces esta semana)
Un phi=+1.0 entre dos modelos distintos fue SIEMPRE el arnes: timeout de 60s con items que tardan 70s; caracter "≡" por stdin en cp1252; digitos señuelo en el enunciado ("n/2", "1..92", "responder -1") que la regex del modelo tomaba como parametros; y filas caidas por API contadas como fallos. Ahora cada fila guarda el texto crudo y la fuente rechazada. Regla: cuando dos modelos coinciden demasiado, sospechar del arnes antes que de los modelos.

## Como llevar lo subjetivo a synth (para el SDLC de 6 etapas)
No se sintetiza la respuesta: se sintetiza el CHEQUEO. El humano etiqueta 3 buenos y 3 malos una vez por chequeo; la funcion se promueve si los separa; corre gratis sobre todo lo que pase por la etapa. Lo que ningun chequeo cubre (gusto, prioridad, "es el producto correcto") queda para juez cross-familia o humano — ahi y solo ahi aplica OneFlow. El hook ETS del 2026-09-08 es un synth promovido a mano: "hablame simple" convertido en 3 reglas medibles y un script.

## Pendiente
- glm-5.2-nothink: 0 filas (rate limit de zhipu + breaker). Es el unico otro modelo sin razonamiento; podria fallar como chat. Reintentar con 1 worker.
- Para medir decorrelacion de codigo hace falta MAS dificultad que 57 tipos algoritmicos con params tipados: tareas multi-funcion, specs ambiguas, o librerias (BigCodeBench-Hard).
