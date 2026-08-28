# Auto-evolución: estado real vs plan (auditoría escéptica)

Fecha: 2026-08-27 · Repo: `~/.claude/orchestration` · Fuentes: GOAL.md, SELF-EVOLUTION-PLAN.md,
`mmorch/{goal,evolve,evolve_findings,auto_repair,automerge,merge_train,hardening,triage,loop_nightly}.py`,
`scripts/nightly.py`, logs reales (`logs/nightly.jsonl`, `evolve_red.jsonl`, `merge_train.jsonl`), git.

## TL;DR

El subsistema **existe, corre todas las noches y produce trabajo real**, pero el "circuito
cerrado autoejecutable" del GOAL **no está cerrado**: hoy es un pipeline
*harvest → propose → sandbox → review-branch/PR → **merge humano***. Ningún cambio a código
existente se auto-aplica. Y — hallazgo central — **el gate de 6 checks de fitness()
(goal_aligned + ensemble + costo + rollback + tamper-halt) NO está en el camino vivo**: el
loop nocturno solo pasa por `zone_of()` + tests. La maquinaria de seguridad "fuerte" está
implementada y testeada, pero el loop de producción usa una versión más débil (y más honesta:
zona roja + suite verde + triage + humano).

---

## 1. Lo planeado (GOAL.md + SELF-EVOLUTION-PLAN.md)

- **GOAL.md** (47 líneas): north star = orquestador auto-evolutivo con auto-aplicación
  *progresiva y reversible*. Invariantes: zona roja con gate humano, cross-family,
  reversibilidad first-class (`rollback()` probado antes de auto-aplicar), `fitness()` de 6
  checks como gate, BudgetKeeper, auditoría `kind="auto_action"` (GOAL.md:15-27).
- **Plan phase-gate**: F0 (GOAL+budget) ✅, F1 (predictor, re-scoped) ✅, F2 (megasource
  pricing) ✅, F3 (rollback+fitness) ✅, F4 (motor self_evolve) ✅, F5 (shadow prior)
  ⚠️ construido pero dormido/data-gated, F6 (backstops) parcial, F7 (MLP router) no
  construido (pre-requisito ≥10k outcomes no cumplido).

Las marcas ✅ del plan son verídicas *como código+tests*, pero varias piezas de F3/F4 son
hoy **maquinaria de museo** respecto al loop que realmente corre (ver §4).

## 2. Código vivo (con evidencia de ejecución)

El driver real es **`scripts/nightly.py`** — Windows Task Scheduler (`mmorch-nightly`
verificado **Running** vía `Get-ScheduledTask`; también `mmorch-server` y `mmorch-autopull`).
No depende de Claude ni de cupo. Encadena, fail-soft:

| Paso | Módulo | Estado |
|---|---|---|
| `nightly_evolve()` | `mmorch/evolve.py:697` | **VIVO** — harvest (`evolve_findings.harvest_findings`, recicla `code_review` sobre archivos cambiados por git log) → `propose_with_fast_retry` (evolve.py:662) → `coordinated_evolve_round` (evolve.py:557) con lock por archivo |
| autoresearch de prompts | `nightly.py:96-120` | VIVO, en worktree, branch `mmorch/ar-*` solo si mejora (8 branches ar-* existen) |
| hardening (tests anti-mutante) | `mmorch/hardening.py` | VIVO, review branch `mmorch/hard-*`, merge humano |
| `auto_repair.repair()` | `mmorch/auto_repair.py:68` | VIVO — repara errores estructurados del propio record nocturno, 1/noche, worktree, gate = suite completa |
| `merge_train.run_train()` | `mmorch/merge_train.py:40` | VIVO — junta las branches amarillas del día en `mmorch/tren-YYYY-MM-DD`, suite sobre la UNIÓN, con triage mecánico previo (`triage.py`, cero LLM) |
| reflexión + digest + idea-loop | `mmorch/loop_nightly.py:199,289,510` | VIVO — auto-reflexión con cifras calculadas (anti-alucinación, loop_nightly.py:157-179), budget en USD reales (cap $3/mes, loop_nightly.py:17) |

**Evidencia operativa (logs, no docstrings):**
- `logs/nightly.jsonl`: **35 corridas**, 2026-07-10 → 2026-08-25.
- Total histórico: **4 PRs/branches abiertos por evolve, 122 candidatos rojos (fitness fail), 25 skips por lock**.
  Los 4 opened son TODOS del 2026-08-25 (`health.py`, `loop_nightly.py`, `stuck_detector.py`, `bughunt.py`).
- **12+ noches con 0 PRs** por dos bugs reales, documentados en el propio código:
  el code-fence del modelo viajaba dentro del .py → SyntaxError → suite roja
  (fix `extract_fence`, evolve.py:640-647) y el `--basetemp` global roto (evolve.py:398-409).
  El loop estuvo semanas "corriendo" sin producir nada — y el sistema de stuck-detection/reflexión
  fue parte de cómo se diagnosticó.
- `logs/merge_train.jsonl`: 7 corridas; trenes 08-23 (4 branches) y 08-25 (6 branches, 1 conflicto
  apartado) con gate **verde**. Las branches `mmorch/tren-2026-08-{22,23,25}` **siguen sin mergear**
  (el click humano está pendiente) — pero `git log --merges` muestra merges manuales de branches
  `mmorch-sbx-*` y `mmorch/ar-*` a la rama de trabajo: el humano SÍ consume el output del loop.
- `logs/evolve_red.jsonl` (13 entradas, 08-21→08-24): el "por qué" de cada fitness fail se persiste
  (evolve.py:589-600) — observabilidad agregada tras medir que diagnosticar era adivinar.

## 3. Gates de seguridad: qué existe DE VERDAD

### 3.1 Tamper-halt de GOAL (existe, verificado, pero cobertura parcial)
- `goal.py:24-60`: `GoalTampered` + `goal_guard()` compara sha256 de GOAL.md contra
  `GOAL.hash` autorizado. **Verificado en vivo**: hash actual `2d2d924b3df25697` == GOAL.hash.
- Cableado en `evaluate()` (evolve.py:112-117) y re-chequeado justo antes del apply de
  `self_evolve()` (evolve.py:341-343, defense-in-depth). Tests reales:
  `tests/test_evolve_goal_guard.py` (propaga, no se traga), `tests/test_goal.py:42-52`.
- Capa extra fuera del repo: `~/.claude/never-edit.txt` líneas 8-10 bloquean
  `**/orchestration/goal.md` y `goal.hash` a las tools de Claude (hook PreToolUse).
- **PERO**: el camino vivo nocturno **nunca pasa por `evaluate()` ni por `self_evolve()`**
  (ver 3.4) → `goal_guard` hoy solo frena los caminos que nadie ejecuta. Si alguien
  envenena GOAL.md, el loop nocturno ni se entera (aunque tampoco consume GOAL.md: no llama
  a `goal_aligned`). El tamper-halt es real pero protege una puerta por la que no pasa nadie.

### 3.2 Zona roja (el gate que SÍ está en el camino vivo)
- `zone_of()` (evolve.py:297-311): paths rojos (`_RED_PATHS` evolve.py:270 — GOAL.md,
  GOAL.hash, .env, goal.py, budget.py, config.py), path-escape, y **scan de contenido**
  `red_content_hits` (evolve.py:276-294) con delta vs baseline (fix del auto-lock medido:
  evolve.py se marcaba rojo a sí mismo por contener las firmas como regex).
- Se ejecuta en `coordinated_evolve_round` (evolve.py:580) ANTES del sandbox, y lo reusa
  `automerge.classify_branch` (automerge.py:37-63) — mismo semáforo, no uno paralelo. Bien.
- Self-check ejecutable al final de evolve.py (líneas 840-848) cubre los 3 casos del delta-scan.

### 3.3 Rollback / reversibilidad
- `rollback()` + `_rollback_roundtrip()` (evolve.py:68-81, 193-209): implementados y
  testeados. **Pero en producción la reversibilidad real es git**: el loop vivo nunca muta
  el repo (worktree + branch, evolve.py:374-431 `sandbox_branch`; "el repo vivo NUNCA se
  toca"). `rollback()` estructural solo corre dentro de `evaluate()` → mismo estatus museo.
  La invariante del GOAL se cumple por una vía distinta (y más fuerte) que la planeada.

### 3.4 fitness() de 6 checks: implementada, testeada, **fuera del loop**
- `evaluate()` (evolve.py:88-143): ast + rollback-roundtrip + goal_guard + goal_aligned
  (cross-family, goal.py:83-106) + ensemble (degradación honesta a 1 familia,
  evolve.py:151-170) + cost fail-closed (evolve.py:173-182) + BudgetKeeper (evolve.py:185-190).
  Tests: `test_evolve_motor.py` (8), `test_evolve_goal_guard.py` (3).
- **Quién la llama en vivo: nadie.** `nightly_evolve → coordinated_evolve_round` va directo
  a `zone_of` + `sandbox_branch` (tests). `mmorch_evolve_self` (mcp_server.py:619) la corre
  pero es **DRY explícito** ("NUNCA aplica"). `self_evolve(do_apply=True)` no tiene ningún
  caller fuera de tests. `promote_branch` (evolve.py:434) tampoco.
- Consecuencia: **`goal_aligned()` — el ancla anti-drift, "el 6to check" — no gatea nada de
  lo que el sistema produce cada noche.** El gate real es: zona + pytest + triage mecánico
  (triage.py) + refutador cross-family de branches (commit c9c73ff: **ADVISORY, no gate**,
  "medido") + humano.

### 3.5 Automerge (el único auto-apply real… que nunca disparó)
- `automerge.try_automerge` (automerge.py:66): carril verde = diff SOLO tests/ o archivos
  nuevos, sin firmas rojas, suite ya verde. Llamado desde auto_repair.py:132 y
  hardening.py:133. Ledger append-only prometido en `logs/automerge_ledger.jsonl`.
- **El ledger no existe en logs/** → `try_automerge` no se ejecutó ni una vez (o cada
  intento quedó en zona ≠ verde antes de escribir… no: el ledger se escribe SIEMPRE,
  automerge.py:108-112). Conclusión: código vivo-por-cableado, **cero ejecuciones reales**.
  El "auto-apply" del sistema es hoy una capacidad latente, no un hecho.

### 3.6 Otros guardrails reales
- Kill-switch `logs/loop_paused` chequeado en loop_nightly.py:518, auto_repair.py:76,
  automerge.py:71, merge_train.py:47. Consistente.
- Budget: cap duro $3/mes + 2000 calls en `loop_nightly.py:16-17` con USD reales
  (`loop_budget.json`); BudgetKeeper global aparte (budget.py, F0).
- Lock por archivo con reap + outcome retroactivo del veredicto humano
  (merge=1.0/rechazo=0.0 → bandit, evolve.py:524-554) — el loop aprende del humano, no
  solo de sus propios gates. Diseño correcto; señal aún escasa (4 PRs).
- Auditoría: `archive_variant` → `logs/evolution_archive.jsonl` **no existe** (nunca se
  llamó: solo lo llama `self_evolve`, museo). La auditoría real es nightly.jsonl +
  evolve_red.jsonl + branch_provenance.jsonl.

## 4. Vivo vs decorativo (resumen)

**Vivo (corre cada noche, con logs):** nightly_evolve, evolve_findings, propose_with_fast_retry,
sandbox_branch, coordinated_evolve_round, auto_repair, hardening, merge_train, triage,
reflexión/digest/idea-loop, zone_of + red_content_hits, kill-switch, budgets, provenance.

**Implementado+testeado pero SIN camino de ejecución real (decorativo hoy):**
`evaluate()` (las 6), `goal_aligned`, `goal_guard` (solo protege caminos muertos),
`rollback()` estructural, `self_evolve()` motor F4, `promote_branch`, `pursue_goal`,
`archive_variant`. `mmorch_evolve_self` MCP existe pero es DRY.

**Latente (cableado, 0 ejecuciones):** try_automerge carril verde.

**Dormido por dato (decisión medida, no bug):** shadow_prior F5 (scale=0; desbloqueado en
teoría con code_embedder +0.168 pero sin outcomes de código reales alimentándolo).

**No construido:** F7 MLP router; F6 parcial (health/fleet existen; cifrado/keyring no
visto en el camino).

## 5. Qué falta para que el "circuito cerrado autoejecutable" sea real

1. **Unificar los dos sistemas de gate.** O el loop nocturno adopta `evaluate()` (al menos
   goal_guard + goal_aligned sobre el diff antes de abrir PR), o se admite en GOAL/plan que
   el gate de producción es zona+tests+triage+humano y se retira fitness() de 6 checks del
   discurso. Hoy el documento de seguridad describe un sistema y corre otro.
2. **Primer auto-apply real**: hacer disparar el carril verde de automerge (hay material:
   hardening produce branches solo-tests) y verificar el ledger. Hasta que
   `automerge_ledger.jsonl` tenga una línea con `merged:true`, la "auto-aplicación
   progresiva" es 0%.
3. **Cerrar el click del tren**: 3 trenes verdes esperan merge humano desde el 08-22. La
   fase 2 declarada del tren (merge-then-monitor con rollback estructural + SPC,
   merge_train.py:9-10) es exactamente el paso que convertiría el pipeline en circuito.
4. **Volumen de señal**: 4 PRs en 35 noches (y recién desde 08-25). El fix del fence/basetemp
   es de hace 2 días; hay que ver 1-2 semanas de PRs sostenidos antes de subir autonomía.
5. **goal_guard en el arranque del nightly** (1 línea en scripts/nightly.py): barato, y el
   tamper-halt pasaría de proteger caminos muertos a proteger el loop real.
6. **Poblar el camino del bandit con outcomes humanos** (ya cableado en reap_merged_prs) —
   necesita que el humano mergee/cierre PRs con regularidad.

## 6. Veredicto

- Fases 0-4 del plan: **código real con tests reales** (63+ tests en los módulos del
  subsistema; suite global ~718). No es vaporware.
- Pero el sistema evolucionó *alrededor* de su propio plan: el motor F4 (`self_evolve`)
  quedó obsoleto frente al pipeline nocturno PR-based, que es más seguro y más honesto —
  y nadie actualizó el plan ni desmanteló el museo. El riesgo no es de seguridad (todo
  termina en review humano; zona roja sí gatea el camino vivo) sino de **integridad del
  relato**: los gates estrella (goal_aligned, tamper-halt, rollback probado, ensemble) no
  gatean nada de lo que ocurre cada noche.
- El loop demostró la propiedad más valiosa: **detectó y diagnosticó su propio bucle muerto**
  (12 noches sin PRs → stuck_detector/reflexión → fixes medidos con aritmética del fence).
  Eso es más evidencia de "sistema que se auto-mejora" que cualquier checkbox del plan.
