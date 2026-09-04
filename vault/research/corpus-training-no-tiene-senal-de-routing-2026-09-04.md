---
applies_to:
- orchestration
date: 2026-09-04
status: applied
tags:
- research
- training-data
- routing
- methodology
- ablation
title: El corpus de training no tiene señal de routing — y las ablaciones que sí valían no se guardaron
---

# El corpus no tiene señal de routing (y por qué no es un problema de volumen)

Medición sobre `training/*.jsonl` para decidir si alcanza para destilar un router
antes de alquilar GPUs. **No alcanza, y agregar volumen no lo arregla.**

## Resultado

| prueba | valor |
|---|---|
| n | 2.873 |
| positivos | 1.912 (66,6%) |
| baseline "siempre sí" | **66,6%** |
| accuracy sabiendo el brazo | ~66,7% (**+0,1 pp**) |
| `deepseek-chat` P(y=1) | 64,7% (n=1.895) |
| `gemini-2.5-flash` P(y=1) | 67,0% (n=701) |
| diferencia | 2,3 pp, SE 2,1 pp → **z = 1,10, no significativa** |

La identidad del brazo no predice la etiqueta. Un clasificador entrenado con esto
no puede superar a decir que sí siempre.

## La causa: los datos no tienen la forma de la pregunta

- **111 contextos distintos, sólo 23 con más de un brazo.**
- El 49% del corpus viene de UNA fuente (`ablation-truth`, todo sobre `ablsym_verify`).

Aprender a rutear necesita *pares*: misma tarea, dos brazos, uno gana. Lo que hay son
tuplas independientes `(brazo, resultado)`. Hay 2.873 filas pero **23 comparaciones**.

No es escasez de datos: es un **diseño de recolección equivocado**. Más volumen del
mismo tipo no mueve la aguja.

## Contaminación

`dpo_pairs.jsonl`: **130 de 336 son literales de test** (`{"rubric":"r","artifact":"x"}`,
`"art"`). Quedan 206 reales. Cualquier medición sobre ese archivo sin filtrar está
inflada un 39%.

## Tiers: volumen sin calidad

| dataset | n | tiers |
|---|---|---|
| `sft_judge` | 2.562 | 100% bronze |
| `router_prefs` | 2.873 | 2.761 silver · **112 gold** |
| `dpo_pairs` | 336 | 100% silver (39% basura) |

De 5.771 ejemplos, **112 son gold**. El resto son etiquetas que el sistema se puso a sí
mismo — entrenar con eso destila el propio sesgo, justo contra lo que advierte el
hallazgo del ~74% de false-refute de jueces LLM en checkable difícil.

## El hallazgo que importa: las ablaciones buenas no se guardaron

`ablation_paired.py`, `ablation_symmetric.py` y `ablation_prompt.py` están **bien
diseñadas**: ground truth computada (etiqueta infalible, sin humano), diseño pareado
(el mismo artefacto lo juzgan los dos verificadores), McNemar sobre pares discordantes,
n=350 con power ~0,8 para 10 pp, semilla fija, deterministas.

El **2026-06-08** corrieron **6.190 llamadas** por **US$ 2,15** — el 30% del gasto de
toda la vida del sistema, en un día.

**Ninguna de las tres persiste resultados** (cero `json.dump` / `write_text` en los tres
archivos). En el vault no hay ninguna nota con esos hallazgos: la única de ablación es
`ablation-18.4-cross-family-FINDINGS.md`, del día ANTERIOR, con n=8 y
`status: inconclusive`.

Las conclusiones se fueron por stdout.

## Qué hacer

1. **Re-correr las tres ablaciones con persistencia.** Son deterministas con semilla
   fija: cuestan los mismos US$ 2,15 y dan el mismo resultado, pero queda guardado. Por
   dos dólares se recupera la respuesta a "¿la verificación cross-family sirve?", que es
   el supuesto fundacional de la arquitectura.
2. **No destilar un router con este corpus.** El techo no es la RAM ni el servidor: son
   los datos.
3. **Si se quiere señal de routing, cambiar la recolección**, no el volumen: correr la
   misma tarea por ≥2 brazos y registrar cuál ganó. El aparato ya existe
   (`ablation_paired`), sólo que no alimenta `training/`.
4. **Filtrar los 130 placeholders** de `dpo_pairs.jsonl` antes de cualquier medición.

## La causa raíz, en una línea

`scripts/export_training_data.py` documenta su propia fuente:

```
logs/feedback.jsonl  ->  router_prefs.jsonl   (arm/pattern/context -> reward)
```

`feedback.jsonl` es el log de recompensas del **bandit**. Un bandit registra "tiré del
brazo A, obtuve recompensa R" — nunca "A contra B sobre la misma tarea". El corpus está
**despareado por construcción**, no por descuido: se está pidiendo aprendizaje de
preferencias a un log que estructuralmente no puede contener preferencias.

Y la única fuente del repo con la forma correcta — `ablation_paired.py`, donde el MISMO
artefacto lo juzgan los dos verificadores — no figura como fuente del exportador. Ni
siquiera persiste su salida.

O sea: el dato pareado que hace falta ya se genera; simplemente se tira, y el que sí se
guarda no puede responder la pregunta.

## Lección de método

Vale tanto como el resultado: **un experimento que no persiste su output no ocurrió.**
Seis mil llamadas de un diseño correcto valen lo mismo que cero si el resultado no
sobrevive a la terminal.
