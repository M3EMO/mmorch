# mmorch cognitive architecture — synthesis (intuition + insight)

Status: **Phases 0–5 BUILT** (`signature.py` baa4013; `intuition.py` 2e7d0ef + 045a6f7;
`record_outcome` forward-wire 045a6f7). Backfilled 1893 logged outcomes cero-cupo → 84 arms/74 sigs.
`decide` (gate), `reframe`/`candidates_pooled` (insight), the learning forward-wire, AND the
router wiring all ship (`5fe4ba0`): `route(models=[...])` + `run_code_task(intuition_models=[...])`
consult the sig-bandit (opt-in, default-off = zero regression); `mmorch_intuition` MCP tool exposes
the decision for A/B. **Only open piece = a DECISION, not a build:** flip intuition ON by default in
the routers, gated on an A/B vs the current default on real traffic. **Upgrade path (deferred):** swap
the tabular bandit for LinTS (linear contextual) — only when the tabular+`candidates_pooled` path
measurably underperforms; it's a drop-in swap of the same record/select/candidates interface.
Consolidates a ~16-source harvest; supersedes the A–U notes.

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
- **Validity test (frame-invariance) = a MONITORING SIGNAL, not a pass/fail gate.** Re-describe → same
  signature? Report a stability SCORE; flag low-stability signatures. It WILL fail sometimes (LLMs are
  surface-sensitive) — that's expected, not fatal (see collision handling below).
- **Surface-collapse is MITIGATED, not prevented.** Two problems can collide on one signature
  (`translate Py→Java` vs `refactor Py for readability` both → TRANSFORM/needs_codebase/correctness).
  The fix is NOT a finer fixed vocab (→ killed dynamic-growth) — it's the candidate-SET + verify design:
  a collision recalls BOTH strategies; execution-verification disposes. Recall is high-recall on purpose;
  precision comes from VERIFY, never from the key. The coarse key is a feature, not a bug.

### 3.2 Association store — DON'T build a new one; RE-KEY the existing bandit/recall
**Cross-family refine (§9) killed the proposed new SQLite store as redundant.** mmorch ALREADY has the
landscape: the bandit + `record_outcome` + `recall` + playbooks ARE the (context→strategy, measured)
store with local updates and no catastrophic forgetting. INTUIT's only net-new artifact = the **KEY**.
- **The change = key the existing recall/bandit by the structural `signature` instead of the raw task
  string.** That's it. No new store, no new learner. The "energy landscape" = the existing measured
  weights; "energy descent" = the existing recall; "Hebbian/MDL" = the existing bandit/consolidation.
- Why it's still worth doing: a signature key GENERALIZES across surface-different-but-structurally-same
  tasks (raw-string keying does not) — that is the entire value, and it's a small change.

### 3.3 Recall (energy descent)
`recall(signature) -> (candidate_strategies, coherence)`
- **Candidate SET, not an answer** (high recall; verification gives precision).
- **Hierarchical / take-the-best**: match COARSE first (op_type) → broad set; add finer fixed
  components (constraint_bits) only when the coarse set's candidates DIVERGE in outcome. Increase
  RESOLUTION over the fixed basis — never invent dimensions (that's the killed dynamic-growth).
- **Attention framing**: signature(Q) vs stored signatures(K) → relevance-weighted blend of
  strategies(V); multi-head = integrator (cue magnitude) + resonator (cue PATTERN) lenses.
- **coherence** = measured activation at this signature (case-count × best-weight) = familiarity.

### 3.4 The gate — START as a one-line threshold; hysteresis DEFERRED
**Cross-family refine (§9): hysteresis/surprise is premature optimization.** Build the one-liner first:
- `coherence >= X` → **commit fast** (cero cupo, no reasoning); else → escalate to **REASON**/**INSIGHT**.
- Add the surprise (prediction-error) term + hysteresis (two thresholds + refractory, anti-thrash) ONLY
  if the one-line threshold MEASURABLY thrashes or mis-commits in logged outcomes. ponytail: don't build
  the bistable gate until the simple threshold's failure is observed.

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
| **INTUIT: signature** | 📐 | NEW (the only real net-new artifact) — `signature(spec)` projection |
| **INTUIT: assoc store** | ✅ | REUSE — bandit + `record_outcome` + `recall` + playbooks; just re-key by signature |
| **INTUIT: recall** | ✅~ | REUSE existing `recall`, keyed by signature; return a candidate SET |
| **INTUIT: gate** | 📐 | NEW but tiny — one-line `coherence >= X`; hysteresis deferred |
| **INSIGHT** | 📐 | NEW (the other real net-new) — impasse → re-representation, residual test |

## 6. Build order (REVISED by the cross-family refine — lean, no waterfall)
- **Phase 0+1 (ship together) — `signature.py` + re-key recall**: `signature(spec) -> Signature`
  projecting the refuted spec onto the fixed vocab, AND wire it as the key into the EXISTING
  `recall`/bandit (no new store). This is the smallest unit that delivers value (generalization across
  surface-different tasks). Self-check: re-description STABILITY score (cheap, cross-family, no outcomes
  needed) + compositional novelty. Frame-invariance = a logged score, not a gate.
- **Phase 2 — candidate-SET recall + coherence**: make the re-keyed recall return top-N strategies +
  a `coherence` familiarity score. Mostly config over existing recall. Self-check: collision recalls
  both strategies (translate vs refactor), coherence rises with case-count.
- **Phase 3 — one-line gate**: `coherence >= X` → commit fast, else escalate. Wire as a pre-step to the
  router. Self-check: commits on familiar, escalates on novel. (Hysteresis/surprise: deferred, build
  only on observed thrash.)
- **Phase 4 — INSIGHT**: impasse (all candidates fail verify) → re-representation (relax a constraint
  bit / re-frame via `perfect`), residual test gates any vocab extension, offline review. Self-check:
  impasse triggers re-represent, residual gates extension.
- **Phase 5 — integrate + evolve**: wire INTUIT into routing + the cooperative workflow; evolve the
  signature vocab / thresholds by measured outcome via `hillclimb` (NOT hand-tuned).

## 7. Non-goals / killed branches (don't rebuild these)
- Dynamic dimension growth per-collision (→ lookup table, never converges, needs causal inference). Dead.
- A `solution_strategy` field IN the key (circular — keys by the output). Dead.
- Token-level / mid-call resume; an arbitrary node/edge graph engine (role-chain covers it).
- A perfect minimal vocabulary up front (reservoir: rich basis + outcome-selection instead).

## 8. Cross-family refine (mmorch ensemble, cero cupo, $0.002) — what it cut
Ran `mmorch_ensemble_verify` (DeepSeek gen, Gemini skeptics, subjective→cross-family) on the build plan.
Unanimous refute (0.9), 4 legit hits → the plan got leaner:
1. **ORDERING**: Phase 0 alone can't validate frame-invariance (needs outcomes) → ship Phase 0+1 together;
   self-check on re-description stability, not outcome-prediction. Frame-invariance = monitor, not gate.
2. **REDUNDANCY (conceded)**: the proposed new assoc_store duplicates the existing bandit/recall/playbooks
   → DON'T build it; just re-key the existing recall by signature. The only net-new = signature.py + INSIGHT.
3. **SURFACE-COLLAPSE (conceded)**: a fixed coarse key WILL collide (translate vs refactor → same sig) →
   that's WHY recall returns a SET + VERIFY disposes; precision never comes from the key. Made explicit.
4. **SMALLER BUILD**: hysteresis/surprise gate is premature → one-line `coherence>=X` first, hysteresis
   only on observed thrash.
Net: the "cognitive architecture" was ~80% already in mmorch; real net-new = the structural KEY + INSIGHT.

## 9. Source provenance (compact)
recall=energy-descent + interference=surface-failure: Hopfield, predictive coding, protein folding ·
structural=invariant: tensor (frame-invariance), Noether · gate=surprise + local-no-forgetting:
predictive coding, Kahneman · memorize→structure→prune=MDL: grokking, the builder-breaker discovery
paper · rich-basis+readout: reservoir computing · integrator/resonator + bifurcation=insight + coherence
gate as threshold/refractory: Izhikevich dynamical-systems · compositional signature + recall=attention:
subword tokenization, transformers · "intuition proposes / verify disposes / recall not precision":
the competitive-programming intuition video + the whole rubric_loop invariant. Validations folded in.
