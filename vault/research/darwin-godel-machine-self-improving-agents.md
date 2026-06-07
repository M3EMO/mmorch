---
title: Darwin Godel Machine self-improving agents
status: applied   # evolve.py subset (fitness+archivo+propose gated) landed; full open-ended sigue gated
confidence: 0.9
verifier: gemini-2.5-flash
tags: [research, autolearning, mmorch, self-evolution]
sources: [https://arxiv.org/abs/2505.22954, https://github.com/lemoz/darwin-godel-machine]
created: 2026-06-07
---
## Qué es
Darwin Gödel Machine (Zhang et al. 2025, ICLR 2026): agente que **modifica su propio código**
Python para ser mejor coding-agent. Evolución open-ended: fitness = **performance empírica en
benchmarks**; selección de padres balancea **performance + NOVELTY** (exploración diversa);
mantiene una **POBLACIÓN/archivo** de variantes (no una sola línea); sandbox + HITL.

## Aplicable a mmorch (suavizado tras refutación)
- Inspiración para `self_evolve`, NO mapeo 1:1.
- **Caveat (cross-family conf 0.90):** el self_evolve actual (audita+propone+gate) es demasiado
  minimal para implicar la maquinaria DGM (evolución poblacional + novelty search). Mapear el DGM
  completo es forzado.
- **Subset aterrizable:** (a) mantener un ARCHIVO de variantes que pasan tests (no solo la última),
  (b) usar **test-suite pass-rate como fitness empírica** (no solo juicio LLM), (c) novelty básico
  para no caer en óptimos locales. Lo PESADO del DGM (auto-modificación open-ended) queda gated/HITL.

## Veredicto cross-family
passed=False conf=0.90 — técnica real; mapeo full exagerado. Aterrizar solo el subset archivo+fitness. Status: seed.
