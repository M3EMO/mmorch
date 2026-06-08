# mmorch — Plan de build hacia el ideal auto-evolutivo
Fuente: brainstorms/2026-06-08-mmorch-ideal-vision.md (grill completo). Estructura: **phase-gate** (cada fase tiene criterio de salida duro). `loop_until_done` se usa SOLO dentro de fases de scope desconocido (marcado ⟳). No es el driver top-level.

## Reglas del plan (invariantes)
- **No se avanza de fase sin pasar su CHECKPOINT** (criterio de salida verificable).
- Cada fase: commit atómico + `pytest tests/` verde antes de cerrar.
- Nada toca zona ROJA sin gate humano (ver brainstorm §zona roja).
- Cada entregable que se auto-aplique debe implementar `rollback()` (Fase 4+).
- ⟳ = sub-loop `loop_until_done` legítimo (scope desconocido, dedup, hasta dry).

---

## FASE 0 — Cimientos: GOAL (ancla anti-drift) + BudgetKeeper (costo)
**Goal:** anclar el norte (anti goal-drift) y que ninguna fase repita el +$5.
**Entregables:**
- ✅ **GOAL.md + `mmorch/goal.py`** (HECHO): contrato north-star + invariantes + non-goals + métricas. `goal_aligned(change)` = verify cross-family del cambio contra el GOAL (refuta si deriva/bloatea/rompe invariante/zona-roja). Modelado sobre el `/goal` nativo (condición + gate que bloquea "done"). Editar GOAL = zona roja (`goal_hash()` audita). 3 tests verdes.
- `BudgetKeeper` (`mmorch/budget.py`): lee `MAX_MONTHLY_USD` de config; antes de cada `call()` chequea el acumulado del mes en metrics.jsonl; si excede → bloquea no-críticas / exige override humano; notifica.
- Contabilidad honesta: calls fallidas (timeout) hoy loggean cost=0 → estimar costo server-side facturado (el gap del +$5).
**Checks/tests:**
- ✅ goal: load_goal/goal_hash deterministas; goal_aligned embebe el GOAL + cross-family.
- test: acumulado > límite → `call()` no-crítica bloquea; override humano permite; suma correcta.
**CHECKPOINT (salir):** goal tests + budget tests verdes + demo: `MAX_MONTHLY_USD=0.01` bloquea un fan_out. Sin esto NO se corre nada que gaste API.

---

## FASE 1 — v0.1 NN: predictor de costo/latencia (tabular, sin red todavía)
**Goal:** decisión informada por costo predicho; cero overfit (target loggeado directo).
**Entregables:**
- `mmorch/predict.py`: LightGBM (o regresión lineal si se evita la dep) sobre metrics.jsonl. Features: (modelo, patrón, in_tokens, log-len). Target: cost_usd, latency_s.
- `predict_cost(model, pattern, prompt)` integrado como señal INFORMATIVA (no vinculante) en route/cascade.
**Checks/tests:**
- cross-val: error < 20% en held-out.
- test: `predict_cost` retorna estimación sana para casos conocidos.
- test: no cambia el comportamiento de route/cascade (solo informa).
**CHECKPOINT:** error cross-val <20% + `predict_cost` expuesto y testeado. (Sin red neuronal aún — eso es Fase 5/7.)

---

## FASE 2 — Megafuente v1: pricing → auto-update config (zona amarilla)
**Goal:** mmorch mantiene sus propios precios; primer acto autodidacta reversible.
**Entregables:**
- `mmorch/megasource.py`: fetcher de fuente ESTRUCTURADA (YAML/repo público/webhook oficial — NO scraping con captcha) de precios provider.
- distill → propone diff a `config.py` (zona amarilla: notifica + reversible).
- auto-drift detection sobre usage propio (metrics+feedback) — `learn` ya lo ve, ahora auto-reacciona proponiendo.
**Checks/tests:**
- test: fetcher parsea fuente mock → (modelo, price_in, price_out).
- test: el diff propuesto a config es válido y revertible (aplicar+rollback).
- test: precio actualizado se refleja en `cost_usd()`.
**CHECKPOINT:** un ciclo completo fetch→propone→(humano aprueba)→aplica→rollback probado, todo auditado en memory.

---

## FASE 3 — Reversibilidad: `rollback()` + `fitness()` (prerequisito del motor)
**Goal:** la maquinaria de deshacer y de aprobar, antes de cualquier auto-aplicación.
**Entregables:**
- `mmorch/evolve.py::rollback(change_id)`: git reset/revert a snapshot previo + tombstone notas + `write_episode(kind="rollback")` + re-correr fitness post-rollback. `change_id` = {diff, snapshot, notas creadas}.
- `mmorch/evolve.py::fitness(change)`: las **6 obligatorias** — pytest 100%, checkers (`python_ast_valid`+`unit_test`), ensemble-AZUL cross-family, rollback probado en sandbox, no-degradación de costo (≤10% verde/≤20% amarillo), **`goal_aligned(change)` passed** (6ta, no determinista — bloquea aunque los tests estén verdes si el cambio deriva del GOAL).
**Checks/tests:**
- test: aplicar un cambio trivial → rollback → estado idéntico (git + memoria).
- test: fitness rechaza un cambio que rompe un test.
- test: fitness rechaza un cambio cuyo rollback falla.
**CHECKPOINT:** rollback restaura estado bit-a-bit en un caso real + fitness bloquea correctamente un cambio malo. **BLOQUEANTE:** ensemble-AZUL necesita 3ra familia (Kimi key) pa ser cross-family real — conseguir key o documentar el gap.

---

## FASE 4 — El motor: `self_evolve()` (loop-until-done legítimo ⟳)
**Goal:** mmorch idea→prototipa→gatea→aplica/revierte→audita, 1 mejora segura por ciclo.
**Entregables:**
- `mmorch/evolve.py::self_evolve()` = SENSE(learn.recommend) → IDEATE(innovate N) → PROTOTYPE(sandbox/worktree) → FITNESS → ZONA(reversibilidad×blast) → APPLY(verde/amarillo)/STOP(rojo) → AUDIT(kind="auto_action") → MONITOR(reward/ECE post) → LEARN(record_outcome).
- batch + `tournament` entre candidatos que aprueban fitness → gana 1; resto a `evolution_archive.jsonl`.
- ⟳ **loop_until_done aquí es correcto**: corre ciclos hasta que K consecutivos no producen mejora segura (dry), dedup contra cambios ya intentados.
- Expuesto como MCP tool `mmorch_evolve_self`; trigger on-idle/cron (daemon recién tras 2 meses sin regresiones).
**Checks/tests:**
- test e2e en sandbox: candidato bueno → aplica + audita; candidato malo → abortado + episodio de fallo.
- test: zona roja detectada → STOP + pide humano (nunca aplica).
- test: dos ciclos → no re-intenta el mismo cambio (dedup).
- test: tournament elige el de mayor fitness; los demás archivados.
**CHECKPOINT:** mmorch aplica SOLO (en sandbox) ≥1 mejora real a su propio código, gateada, auditada, y reversible — con un caso de zona-roja correctamente frenado. Este es el hito #1 de los 6-meses.

---

## FASE 5 — v0.2 NN: shadow prior (k-NN + logístico), scale 0→0.3
**Goal:** la NN empieza a primear al bandit, sin riesgo (shadow + scale gated).
**Entregables:**
- prior shadow: k-NN sobre memoria episódica (embedding del prompt) + logístico simple → `alpha/beta_prior` al bandit.
- scale arranca 0; auto-ajuste dentro de [0.1, 0.8] (zona amarilla, ±0.1 si mejora >2% offline); subir el TOPE = sugerencia a humano.
**Checks/tests:**
- test: con scale=0 el prior no cambia decisiones (idéntico a bandit puro).
- test: held-out — el prior mejora reward; si no, scale no sube.
- test: scale respeta límites [0.1, 0.8]; superarlos requiere gate.
- exploración mínima 10% bandit-puro verificada.
**CHECKPOINT:** prior supera bandit puro ≥5% reward acumulado en 3 ventanas de 200 outcomes → se activa scale=0.3. (Criterio de salida de la spec Q5.)

---

## FASE 6 — Backstops 2da capa (resiliencia + acceso)
**Goal:** lo que falta pa "asistente personal seguro".
**Entregables (priorizados):**
- **Privacidad**: cifrar notas (`cryptography`), claves (`keyring`); anonimizar prompts si se loggean.
- **Provider failover**: health-check por modelo + circuit breaker + redirección (ojo OneFlow → 3ra familia).
- **Regresión gradual**: media móvil de reward; pendiente negativa 3 ventanas → alarma + sugerir rollback.
- **UI/CLI**: `mmorch chat`/`task` o Telegram — mínimo pa interactuar sin ser programador.
- `mmorch doctor`: verifica entorno (fastembed, duckdb, python en PATH).
**Checks/tests:** test por cada uno (cifrado round-trip, circuit breaker abre tras N fallos, alarma de pendiente, doctor detecta dep faltante).
**CHECKPOINT:** cada backstop con su test verde. Decisión pendiente: single-user vs multi (afecta privacidad/tenancy).

---

## FASE 7 — v1.0 NN: MLP híbrido 100k-200k (target largo plazo)
**Goal:** el router neuronal completo de la spec Q4.
**PRE-REQUISITO (criterio de entrada):** ≥10.000 outcomes etiquetados de ≥5 dominios distintos (math, código, soporte, finanzas, planificación) — lo provee el loop (Fase 4) corriendo + uso real. **NO se construye antes** (overfit single-domain).
**Entregables:** MLP (181-500 dims → 256 → 128 → cabezas quality/uncertainty/threshold), PyTorch/JAX, `logs/nn_router.pt`; entrena en zona verde (sandbox), promueve en amarilla, auto-revert si ECE prod >0.15.
- ⟳ NAS-lite: la arquitectura misma se evoluciona vía Fase 4 (innovate arquitecturas → sandbox-train → fitness → promueve).
**Checks/tests:** curva de validación estable > prior v0.2 durante 10 épocas; ECE <0.10; rollback de modelo probado.
**CHECKPOINT:** MLP supera al prior v0.2 en held-out + calibrado + reversible. Recién acá la NN "grande" es producción.

---

## Cómo se ejecuta el plan
- **Top-level = phase-gate manual** (vos/Opus avanzás fase tras checkpoint verde). NO loop_until_done arriba.
- **Dentro de fases**: `loop_until_done` legítimo en Fase 4 (evolucionar hasta dry) y en cualquier "fix hasta tests verdes".
- **Dogfooding**: desde Fase 4, mmorch puede manejar partes de Fases 5-7 con su propio loop — gateado, en sandbox, zona verde/amarilla.
- **Orden duro**: 0 (budget) → 3 (rollback/fitness) → 4 (motor) son la columna vertebral de seguridad; 1/2/5/6/7 cuelgan de ahí.
