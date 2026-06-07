---
title: LLM-as-jury ensemble y errores correlacionados
status: applied   # ensemble_verify min_veto (minority-veto) landed; mas-familias pendiente
confidence: 0.9
verifier: gemini-2.5-flash
tags: [research, mmorch, verification, ensemble]
sources: [https://arxiv.org/html/2604.22891v2, https://arxiv.org/pdf/2603.00039]
created: 2026-06-07
---
## Qué es
LLM-as-jury: panel de múltiples jueces, decisión por **voto mayoría**. Baja el self-preference
bias **>50%** (GPT-4o 82%->30%). PERO el voto mayoría NO corrige sesgos SISTEMÁTICOS/correlacionados.
Confounder-aware aggregation modela los confounders latentes compartidos que inducen **errores
correlacionados entre jueces**. Variante minority-veto (pocos vetos -> invalid) sube true-negatives.

## Aplicable a mmorch (suavizado tras refutación)
- Valida `ensemble_verify` (I-3): K escépticos + voto reduce el caso "1 verificador no lo vio".
- **Caveat (cross-family conf 0.90):** decir que cross-family "ES LA decorrelación correcta" exagera.
  Cross-family REDUCE error correlacionado (arquitectura distinta) pero NO lo elimina (data/RLHF
  compartidos pueden persistir). Es defensa parcial, no total.
- **Acción:** (a) preferir verificadores de FAMILIAS distintas dentro del ensemble (hoy 2 google = subóptimo;
  activar más familias), (b) ofrecer modo **minority-veto** además de mayoría, (c) documentar el residual correlacionado.

## Veredicto cross-family
passed=False conf=0.90 — el principio (cross-family + jury) es sólido y aplicable; solo el "elimina todo"
se refutó. Status: verified (la mejora aplicable es clara: minority-veto + más familias).
