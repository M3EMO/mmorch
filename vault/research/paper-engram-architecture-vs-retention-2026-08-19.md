---
title: Synaptic engram architecture (Science 2026) — candidata para retention.py
status: seed
tags: [research, mmorch, memory, retention, neurociencia]
created: 2026-08-19
sources: [10.1126/science.aee7004]
---

Paper: "Artificial hibernation reveals synaptic engram architecture
associated with memory retention" (Science, DOI 10.1126/science.aee7004).

## Hallazgo del paper

En hibernación artificial de ratones, las neuronas del hipocampo bajan
mucho su actividad y eliminan gran parte de sus espinas dendríticas y
sinapsis — pero la memoria y las representaciones neuronales asociadas
quedan intactas. Lo que sobrevive NO son las espinas más grandes (la
hipótesis obvia) sino un subconjunto específico: espinas en contacto con
botones **multisinápticos** (que también conectan con otras neuronas). O
sea: lo que predice qué sobrevive es la posición de la espina en una
estructura de red compartida, no su tamaño ni su uso individual.

## Por qué esto es relevante para mmorch, no solo temático

`mmorch/retention.py` (`importance()`, `rank_score()`) hoy pondera
retención de una nota por: recencia (decay exponencial Ebbinghaus),
`access_count` propio (frecuencia), y `open_loop` (Zeigarnik). Los tres
miran a la nota AISLADA — nunca si esa nota es un hub que otras notas citan
o de la que otras decisiones dependen. Es exactamente la hipótesis que el
paper refuta como insuficiente (tamaño/uso individual) frente a la que
confirma (posición estructural compartida).

## Graft candidato (no construido, solo señalado)

Sumar un término de centralidad estructural a `importance()`/`rank_score()`:
una nota citada por `[[wikilinks]]` de OTRAS notas, o referenciada como
`sources:` de varias entradas, resiste el olvido MÁS de lo que su propio
`access_count` predeciría — aunque nadie la haya tocado directamente hace
tiempo, si sostiene la estructura de lo que sí se usa, no debería
tombstonearse igual de rápido.

Analogía honesta, no receta: es un principio biológico (conectividad
compartida > tamaño/uso individual), no un algoritmo para copiar. El mapeo
concreto a software sería un grado de entrada en el grafo de citas del
vault — parecido en espíritu al `co_change_pairs`/`import_graph` que
`mmorch/architecture.py` ya calcula hoy para código, aplicado a notas en
vez de módulos.

No implementado — requiere decidir de dónde sale el grafo de citas del
vault (wikilinks reales vs. co-ocurrencia en decision_mining) antes de
tocar la fórmula de retención. Candidata para grilling si se retoma.
