# Handoff — 2026-06-09

## Goal
Evolucionar mmorch (orquestador multi-modelo, ahorra cupo Claude) hacia el ideal
auto-evolutivo/seguro/barato del brainstorm, construyendo fase por fase, todo goal-gated.

## State
- **Done (esta sesión, ~50 commits, 157 tests, 32 módulos, 19 MCP tools):**
  - Feedback loop revivido (calibration n=1→1001), gating calibrado (#3), policy task-aware (#2),
    bandit contextual (#4), ensemble margin (#5), fan_out coverage honesta (#6).
  - **Ancla GOAL** (`goal.py`): `goal_aligned` (cross-family) + `pursue_goal` (retry) + `goal_guard`
    (tamper-halt) + `GOAL.md`/`GOAL.hash`. Modelado sobre `/goal` nativo. = check #6 de fitness.
  - **BudgetKeeper** (`budget.py`, env `MMORCH_MAX_MONTHLY_USD`, wireado en providers.call).
  - **Motor auto-evolución** (`evolve.py`): Change/rollback/evaluate(6 checks)/zone_of/self_evolve/
    tournament + **sandbox_branch** (git worktree → promote_branch/PR). MCP `mmorch_evolve_self` (dry).
  - **Checkers deterministas** (`checkers.py`, 20): arithmetic/determinant/sql/units/sympy/checksum/
    regex/.../ **code_quality (radon)** + **mutation_score** + **coverage** + **deterministic**. Sandbox
    endurecido (PYTHONHASHSEED=0, TZ=UTC).
  - **Fábrica** (`factory.py`) + **dataset miner JIT-defect** (`dataset.py`) + `build_dataset_repos.py`
    → 10,847 funciones etiquetadas (logs/codequality_dataset.jsonl, gitignored).
  - **predict.py** (v0.1 cost/lat, p90), **megasource/prices** (Fase 2), **nodes.py** (registry orquesta).
  - Docs: `SELF-EVOLUTION-PLAN.md` (Fases 0-4 ✅, 5-7 ⬜ + BACKLOG seeds), `ALGORITHMS-MAP.md`.
- **Hallazgos clave (con datos):** LLM-verify de math/código dura ≈ azar/74%-false-refute → tool-verify
  determinista. Code-quality desde texto (estructural + bge-small) = AZAR aun a 10k → cuello es
  representación/framing, no escala. goal_aligned tiene varianza (false-refutes en checkers puros).
- **Blocked:** PUSH (no remote, no gh). Kimi key (ensemble-AZUL cross-family real). headroom (descartado,
  lossy + no instala bien; eval_headroom*.py quedan).

## Next
1. Resolver push: usuario crea repo GitHub + da URL → `git remote add` + push. (o instala gh)
2. Continuar plan: **Fase 5 (NN shadow prior)** O el **flywheel real**: entrenar `model:code_embedder`
   (SimCLR) en WSL+torch sobre el dataset de 10k (signal = EJECUCIÓN/checkers, NO imitación LLM).
3. Limpieza disco pendiente: `.dataset_repos/` (~cientos MB) + cache fastembed + WSL `~/hrvenv`.

## Decisions
- Orquestador determinista core; modelos grandes = NODOS que la FÁBRICA entrena en WSL (conductor ≠ orquesta).
- Algoritmos: pull-on-demand cuando un problema MEDIDO lo pide (anti scope-creep), no batch.
- goal_aligned = 1 voto junto a deterministas, NO árbitro único (su varianza lo exige).
- Generador semántico no-LLM = seed futuro; hoy LLM genera + checkers entienden (oráculo en VERIFY).

## Read first
- `SELF-EVOLUTION-PLAN.md` (fases + backlog seeds), `GOAL.md` (contrato), `ALGORITHMS-MAP.md`.
- `brainstorms/2026-06-08-mmorch-ideal-vision.md` (visión completa grilleada).
- `mmorch/nodes.py` (orquesta), `mmorch/evolve.py` (motor), `mmorch/checkers.py` (batería).
- Memoria mmorch `kind="seed"` (recall): physics engine, ML algorithms, flywheel, semantic generator.
