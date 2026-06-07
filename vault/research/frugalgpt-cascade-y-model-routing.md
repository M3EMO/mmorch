---
title: FrugalGPT cascade y model routing
status: applied   # cascade() landed; threshold-optimizer aun seed
confidence: 0.8
verifier: gemini-2.5-flash
tags: [research, autolearning, mmorch, routing, cost]
sources: [https://arxiv.org/abs/2305.05176, https://openreview.net/pdf?id=qYI4fw3g4v]
created: 2026-06-07
---
## Qué es
FrugalGPT (Chen et al. 2023): cascade multi-modelo. Consulta el modelo más chico primero,
un *generation scorer* (0..1) evalúa la respuesta; si supera umbral, devuelve; si no, escala
al siguiente. Los umbrales se aprenden como **optimización con restricción de budget**
(max calidad s.t. costo <= budget). Reportan hasta **98% de reducción de costo** matcheando GPT-4.

## Aplicable a mmorch (suavizado tras refutación)
- `route` (I-2) YA es el escalón-1 de esto (barato responde + self-score + escala). La extensión
  es CASCADE multi-paso: deepseek-chat -> gemini-flash -> (flag a Opus).
- **Caveat (cross-family conf 0.80):** que `learn` "tunee umbrales" overreachea — learn HOY solo
  recomienda. FrugalGPT necesita un OPTIMIZADOR de umbrales con restricción de budget = componente
  NUEVO a construir (no es learn actual). Además el self-reported confidence de route es más débil
  que un scorer entrenado.
- **Acción:** construir `cascade()` + un threshold-optimizer alimentado por metrics.jsonl. Roadmap, no done.

## Veredicto cross-family
passed=False conf=0.80 — hecho real y relevante; el mapeo directo a learn exagera. Status: seed.
