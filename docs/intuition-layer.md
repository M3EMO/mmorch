# mmorch cognitive architecture — synthesis (intuition + insight)

Status: **DESIGN** (no code yet). Consolidates a ~16-source problem-solving harvest into one
buildable spec. Supersedes the scattered A–U notes in the `intuition-layer` memory.

mmorch is already ~60% a cognitive architecture (FRAME, REASON, VERIFY, MEMORY, most of
EXEC-discipline exist). The real gap = the **INTUIT** component + the **INSIGHT** path. This doc
specs those + the loop that ties them together.

---

## 1. Three principles (what the whole harvest converged on)
1. **Recall = ENERGY DESCENT, not search.** A cue (the framed problem) settles to the nearest
   attractor (the best-fit strategy). No exhaustive search. (Hopfield, predictive coding, protein
   folding, dynamical-systems attractors, grokking.)
2. **Structural = INVARIANT; surface = interference.** The real signature is what's CONSERVED under
   re-description (re-wording, re-coordinatization) — Noether/frame-invariance. Surface-correlated
   signatures' energy wells MERGE → "weird in-between" recall = the failure mode. (Tensor, Noether, Hopfield.)
3. **Intuition PROPOSES, verification DISPOSES; the gate fires on SURPRISE.** Recall is coarse/high-recall;
   execution+cross-family verify gives precision. The gate spends reasoning only on prediction-error
   (surprise), not on the predictable. (Kahneman, predictive coding, the rubric_loop invariant.)

## 2. The loop (how mmorch thinks)
```
FRAME → INTUIT → (surprise/coherence gate) → VERIFY → done + LEARN
   │        │ predictable: cheap commit          │ impasse (low coherence / all candidates fail)
   │     REASON (System 2, Opus) ←───────────── INSIGHT (re-represent = bifurcation) ──┐
   │        └──────────────────────── loop ◄──────────────────────────────────────────┘
   └ EXEC-DISCIPLINE (ROI / cache-by-signature / checkpoints) governs spend throughout
```
Everything is energy minimization: settle to the lowest-energy (best-fit, verified) strategy.

## 3. INTUIT — concrete schema (the new build)

### 3.1 Signature (the structural cue)
Projected MECHANICALLY from the perfectioner's already-refuted spec (`mmorch_perfect`/`build_spec`),
NOT from raw text. Goal-keyed (handles "same problem, different reason"). FIXED but RICH/over-complete
vocabulary (reservoir: don't engineer the minimal perfect set — make it rich, let weights select).
COMPOSITIONAL (subword-style): a novel problem's signature = a composition of known sub-features.
```
Signature = {
  op_type:        GENERATE|TRANSFORM|VERIFY|RANK|SEARCH|DECIDE|REPAIR   # from goal verb + input state
  complexity:     clear|complicated|complex|chaotic                    # Cynefin (mmorch already computes)
  constraint_bits: {has_executable_truth, correctness_critical, ambiguous_goal,
                    needs_exploration, cost_sensitive, multi_step, ...} # from the spec's constraints
  grounding:      self_contained|needs_codebase|needs_fresh_knowledge|needs_tools
}
```
- The recalled value = an orchestration PATTERN (domain-general); the concrete TOOL = a separate
  mechanical artifact-type→checker binding (code→pytest, math→proof-checker).
- **Validity test (frame-invariance):** re-describe the problem → same signature? If it changes,
  it's surface → distrust. Cross-family check on the PROJECTION is the substitution guard.

### 3.2 Association store (the energy landscape) — SQLite, `workflow.db` pattern
```
signatures(sig_id, fields_json, ...)
associations(sig_id, strategy_id, success, fail, last_ts)   # weight = measured success-in-context
strategies(strategy_id, kind, pattern, ...)
```
- **Learning = LOCAL Hebbian/bandit**: each association's weight = co-occurrence of (signature, a
  strategy that VERIFIABLY worked). Updated locally per outcome — NO global gradient → NO catastrophic
  forgetting (a new association never overrides old ones). Reuses `record_outcome`/the bandit.
- **Energy** of a strategy for a signature ≈ −(its measured success weight); recall = descend to
  lowest energy = highest-weight candidates.
- **MDL keep / prune** (grokking cleanup): keep an association OR extend the vocabulary ONLY if it
  COMPRESSES the accumulated evidence better (model-size + error-size, Occam). Prune memorized crutches.

### 3.3 Recall (energy descent)
`recall(signature) -> (candidate_strategies, coherence)`
- **Candidate SET, not an answer** (high recall; verification gives precision).
- **Hierarchical / take-the-best**: match COARSE first (op_type) → broad set; add finer fixed
  components (constraint_bits) only when the coarse set's candidates DIVERGE in outcome. Increase
  RESOLUTION over the fixed basis — never invent dimensions (that's the killed dynamic-growth).
- **Attention framing**: signature(Q) vs stored signatures(K) → relevance-weighted blend of
  strategies(V); multi-head = integrator (cue magnitude) + resonator (cue PATTERN) lenses.
- **coherence** = measured activation at this signature (case-count × best-weight) = familiarity.

### 3.4 The gate (surprise + coherence, hysteretic)
- High coherence / low surprise (intuition predicts well) → **commit fast** (cero cupo, no reasoning).
- Low coherence / high surprise (prediction error) → escalate to **REASON** (Opus) or **INSIGHT**.
- **Hysteretic** (bistable, two thresholds): flip to fast-path needs HIGH coherence; flip back needs
  coherence BELOW a lower threshold → stable mode-commitment + anti-thrashing. A refractory period
  after committing (don't re-decide every step).

## 4. INSIGHT — impasse → re-representation
On impasse (UNKNOWN / low coherence / ALL candidates fail verification):
- DON'T reason harder on the same frame. **Re-represent = a BIFURCATION** (change the phase-portrait):
  relax a constraint bit → recall NEIGHBOR signatures' candidates (constraint relaxation); OR re-run
  `mmorch_perfect` with a different framing → a different signature (representational change).
- **Residual test (left-Kan / discovery):** a genuinely new vocabulary dimension is real ONLY if old
  evidence has un-mappable RESIDUAL under it (else it's relabeling). Vocab extension = MDL + residual,
  **deliberate offline review** (NOT runtime growth, NOT cheap-model causal inference).
- Incubation = the multi-round loop + idle/background (autoresearch).

## 5. What exists vs what's missing
| component | status | mmorch piece |
|---|---|---|
| FRAME | ✅ | `mmorch_perfect`/`build_spec` (goal extraction + cross-family refute) |
| REASON (System 2) | ✅ | Opus escalation; Cynefin/`route` |
| VERIFY | ✅ | truth=execution + cross-family refute (the core invariant) |
| MEMORY/LEARN | ✅ | bandit/ShadowPrior, `record_outcome`, memory/recall/playbooks |
| substrate | ✅ | `workflow_store` (blocks/checkpoints), `hillclimb` (the MDL-ish keep loop) |
| EXEC-discipline | ✅~ | budget_policy, checkpoints/open_loops, cache-by-prefix |
| **INTUIT: signature** | 📐 | NEW — `signature(spec)` projection + frame-invariance check |
| **INTUIT: assoc store** | 📐 | NEW — energy landscape (SQLite), local Hebbian/bandit weights, MDL prune |
| **INTUIT: recall + gate** | 📐 | NEW — hierarchical candidate recall + coherence + surprise/hysteresis gate |
| **INSIGHT** | 📐 | NEW — impasse → re-representation (constraint relaxation / reframe), residual test |

## 6. Build order (each phase = pure module + self-check → wire → verify → commit, the proven graft pattern)
- **Phase 0 — `signature.py`**: `signature(spec) -> Signature` projecting the refuted spec onto the
  fixed structural vocab. Self-check: frame-invariance (re-describe → same signature), compositional novelty.
- **Phase 1 — `assoc_store.py`** (extend `workflow_store`/SQLite): associations + measured weights +
  LOCAL update (reuse `record_outcome`/bandit) + MDL prune. Self-check: Hebbian update, prune-on-no-compression.
- **Phase 2 — `recall.py`**: `recall(signature) -> (candidates, coherence)` — hierarchical/coarse-first,
  candidate SET, attention-relevance. Self-check: coarse→fine divergence, coherence score.
- **Phase 3 — the gate**: surprise/coherence/hysteresis → route fast-commit vs reason. Wire as a
  pre-step to the existing router. Self-check: hysteresis (two thresholds), surprise escalation.
- **Phase 4 — INSIGHT**: impasse → re-representation (constraint relaxation, re-frame via perfect),
  residual test, offline vocab review. Self-check: impasse triggers re-represent, residual gates extension.
- **Phase 5 — integrate**: wire INTUIT into routing + the cooperative workflow; evolve workflow specs
  via `hillclimb` (autoresearch over the orchestration spec).

## 7. Non-goals / killed branches (don't rebuild these)
- Dynamic dimension growth per-collision (→ lookup table, never converges, needs causal inference). Dead.
- A `solution_strategy` field IN the key (circular — keys by the output). Dead.
- Token-level / mid-call resume; an arbitrary node/edge graph engine (role-chain covers it).
- A perfect minimal vocabulary up front (reservoir: rich basis + outcome-selection instead).

## 8. Source provenance (compact)
recall=energy-descent + interference=surface-failure: Hopfield, predictive coding, protein folding ·
structural=invariant: tensor (frame-invariance), Noether · gate=surprise + local-no-forgetting:
predictive coding, Kahneman · memorize→structure→prune=MDL: grokking, the builder-breaker discovery
paper · rich-basis+readout: reservoir computing · integrator/resonator + bifurcation=insight + coherence
gate as threshold/refractory: Izhikevich dynamical-systems · compositional signature + recall=attention:
subword tokenization, transformers · "intuition proposes / verify disposes / recall not precision":
the competitive-programming intuition video + the whole rubric_loop invariant. Validations folded in.
