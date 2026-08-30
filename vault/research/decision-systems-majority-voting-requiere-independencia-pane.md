---
title: Decision systems: majority voting requiere independencia — panel same-model no decorrelaciona
created: 2026-08-29
tags: [research, orchestration, research, decision-theory, ensemble, verification, evolve, llm-as-judge]
status: applied
confidence: 0.9
sources: [https://en.wikipedia.org/wiki/Condorcet%27s_jury_theorem, https://arxiv.org/html/2605.29800, https://arxiv.org/abs/2404.18796, https://arxiv.org/html/2607.08065]
---
## Pregunta

¿Es correcto, como práctica de sistemas de decisión, arreglar un verificador LLM
demasiado estricto pidiéndole el MISMO modelo 3 veces y tomando mayoría? (evaluado
en caliente sobre `mmorch/evolve.py::_ensemble_check`, el gate pre-PR del loop
nocturno — ver [[production-ready 2026-08]]).

## General: Condorcet Jury Theorem

El teorema clásico dice que un panel de votantes cada uno mejor que el azar (p>0.5)
converge a la respuesta correcta con N grande — **bajo el supuesto de independencia**.
Ese supuesto es el que hace todo el trabajo: la literatura (Condorcet's jury theorem,
Wikipedia; "Information pooling through majority-rule voting... with correlated
votes", ScienceDirect) muestra que la efectividad del voto mayoritario **decae** con
la correlación entre votantes, y con correlación positiva suficientemente fuerte se
pierde incluso la monotonicidad (agregar votantes puede empeorar, no solo estancar).
Wisdom-of-crowds (Surowiecki) exige diversidad + independencia + descentralización —
sin eso no hay "sabiduría", hay eco.

## Específico a agentes de IA: LLM-as-judge panels

- **PoLL** (Verga et al. 2024, arXiv:2404.18796): panel de modelos CHICOS de familias
  DISTINTAS supera a un juez único grande, 7x más barato — pero el paper es explícito:
  la ventaja viene de "disjoint model families", no de repetir un modelo.
- **"Nine Judges, Two Effective Votes"** (arXiv:2605.29800, 2026): mide el problema de
  raíz. Con 9 jueces de 7 familias DISTINTAS, el panel completo apenas empata al mejor
  juez individual en algunos datasets. Introduce **n_eff** (votos efectivos, fórmula de
  Kish: `n_eff = k / [1 + (k-1)·φ̄]`) — con φ̄=0.391 de correlación media, 9 jueces dan
  n_eff≈2.18. Repetir el MISMO modelo (φ≈1) da **n_eff≈1**: cero ganancia real, solo
  costo y una falsa sensación de robustez.
- **Self-consistency** (Wang et al. 2022) SÍ funciona — pero para razonamiento con
  ground-truth computable vía múltiples CAMINOS de inferencia (matemática, lógica). No
  aplica igual a un JUICIO de política/cautela: si el sesgo es "veo 'auth'/'security'
  en el diff → refuto" (prior entrenado, no ruido de muestreo), las 3 muestras
  reproducen el mismo sesgo. La auditoría de self-consistency (arXiv:2607.08065)
  confirma: "voting reduces variance, not bias" — y peor, cuando el sesgo es
  sistemático, el voto da "falsa confianza" (el operador ahora confía en algo que
  sigue mal).

## Veredicto sobre la práctica evaluada

**Incorrecta.** Repetir el mismo verificador N veces y pedir mayoría no es un ensemble
válido — es un solo voto disfrazado de N. Medido en el propio caso: 5 branches de
`server.py` rechazadas por 1 verificador (Google, único activo) eran fixes de
seguridad genuinos (validación de input, un path-traversal real) revisados a mano;
un panel same-family de 3 hubiera reproducido el mismo rechazo con más costo y más
falsa confianza, no menos.

## Fix aplicado (híbrido real)

`mmorch/evolve.py::_ensemble_check` (zona roja, cambio revisado por humano, no
auto-aplicado): antes de pagar el verificador LLM degradado, un chequeo
DETERMINISTICO (`_diff_only_adds_guards`, 0 sesgo, 0 costo API) auto-pasa el patrón
más común medido — el diff no pierde ningún contenido existente (tolera
re-indentación: envolver una línea en `try/except`) y agrega al menos una guard
clause de rechazo (`return ... status_code=40x` / `raise`). Backtest contra las 5
branches reales de anoche: 1/5 pasa determinísticamente (el resto sí modifica líneas
existentes más allá de envolver — correctamente cae al chequeo LLM de siempre, ahora
UN solo llamado honesto, sin inflar con copias del mismo verificador).

## Próximo paso (no bloqueante)

Lo que de verdad decorrelacionaría el resto es activar una SEGUNDA familia de
verificador (Kimi hoy inactivo) — eso es lo que el paper PoLL y la nota
[[llm-as-jury-ensemble-y-errores-correlacionados.md]] ya venían pidiendo. El panel
same-model NO es un sustituto válido de eso.
