# 07 — Guardrails y presupuesto del loop

Type: grilling
Status: resolved
Blocked by: 02, 03, 05

## Question

Límites del sistema autónomo: presupuesto API nightly (USD/mes tope),
kill-switch simple (¿archivo flag? ¿variable?), degradación cuando DeepSeek/
Gemini caídos (skip silencioso vs aviso), y qué NUNCA hace solo (no toca código,
no crea PRs, no manda nada externo — solo propone). Además: métricas mínimas de
salud del loop para el resumen matutino (propuestas/semana, tasa aceptación,
notas huérfanas).

## Answer

Grilling 2026-08-12.

- **Presupuesto: USD 3/mes** tope para el loop nocturno de ideas (adjudicación
  + batch + narración), trackeado con `budget.py`. Al tocarlo se frena solo —
  es red contra descontrol, no restricción operativa (~centavos/noche real).
- **Kill-switch**: archivo flag `logs/loop_paused` — existe = nightly saltea
  todo el loop de ideas. Se crea/borra a mano o con "pausá el loop" en sesión.
- **Degradación**: DeepSeek o Gemini caídos → skip de la corrida + línea en el
  digest ("loop no corrió anoche: X caído"). NUNCA degradar a single-family
  (invariante refutador cross-family).
- **Nunca solo**: no mergea, no pushea, no manda nada externo, no toca
  roadmap.md ni código fuera de worktrees sandbox. Solo propone, adjudica y
  prepara.
- **Métricas de salud** (línea semanal en el digest, lunes): propuestas/semana,
  tasa de aceptación, mergeadas vs abandonadas, notas huérfanas, candidatas
  vencidas.
