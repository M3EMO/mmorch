# Handoff — 2026-09-02 — physical/algorithmic discovery prototype

## Goal
FAE: one evolutionary loop over software (local rule) and hardware (topology +
substrate parameters), scored by task error plus data-movement, not TFLOPS.
Test whether that pair outperforms isolated search under executable physics
and equal budgets.

## State
- Done: nonlinear resistive relaxation model in `prototype.py`.
- Done: batched damped-Newton solver; energy decreases, unique equilibrium, residual
  below 7e-12 in observed winners.
- Done: topology/parameter hillclimb with fixed executable scorer and ±5% component noise.
- Done: seed 7 held-out MSE improved `0.10244 → 0.00261`; independent seed also passed.
- Done: equal-budget controls in `prototype.py`: co-design, topology-only,
  parameters-only and random search.
- Done: three-seed control verdict in `VERDICT.md`; co-design won all initial cases.
- Done: multi-family/multi-size/OOD benchmark in `benchmark.py`.
- Done: benchmark covered 3 families × 2 sizes × 3 seeds = 18 cases, 200 evaluations/arm.
- Done: co-design won 12/18 OOD cases; median ratio to best control `0.682`; p90 `1.317`;
  predeclared gate passed, but evidence is heterogeneous.
- Done: typed free/nudged-phase rule executor in `learning_rules.py`.
- Failed honestly: first “discovered” rule was `neg(sign(phase))`, exactly identical
  to the seeded `sign_eqprop` control.
- Failed honestly: reported 9–0 was dictionary-order tie-breaking; both mean OOD MSE
  were exactly `0.0115234169`.
- Done: learning-rule correction (LR in genome, equal meta budget, canonicalize,
  ties, random start). Tuned `sign_eqprop` mean OOD `0.01476` vs evolved
  `0.02771`. Novelty false; pipeline fair. See `LEARNING_RULE_VERDICT.md`.
- Done: movement-aware search in `movement.py` (seed 7, 160 evals/arm, repro
  check). Gate passed: hops `18240 → 14400`; held-out MSE `0.00307 vs 0.00335`.
  One seed, small delta; not wall-plug joules.
- Done: FAE four-arm prototype in `fae.py` (24 evals/arm, 4 cases). Gate
  failed: FAE 1/4 wins, median ratio `2.26`. `hw_only` won 3/4.
  See `FAE_VERDICT.md`.

## Next
FAE pair did not beat hardware-only under the predeclared gate. Do not
scale budget to force a win. Honest options: stop the pair claim, or
predeclare a new experiment (more software mutations, or EqProp as an
allowed primitive inside FAE — different test). Hardware movement still
needs more seeds before any joule claim. Echo / photonic GPU stay dropped.

Software-half correction is done; novelty remains false. Do not claim a
discovered rule.

## Verification commands
- Base prototype:
  `.venv\Scripts\python.exe .scratch\physical-codesign-prototype\prototype.py`
  — expected exit 0, reproducibility check passes.
- OOD benchmark:
  `.venv\Scripts\python.exe .scratch\physical-codesign-prototype\benchmark.py`
  — expected exit 0 and `"gate": true`.
- Data-movement eval:
  `.venv\Scripts\python.exe .scratch\physical-codesign-prototype\movement.py`
  — expected exit 0 and `"gate": true` (fewer hops than mse-only, held-out
  MSE within 2×, still beats the initial net). Exit 2 is an honest miss.
- Learning-rule eval:
  `.venv\Scripts\python.exe .scratch\physical-codesign-prototype\learning_rules.py`
  — expected exit 0 (equal meta budget). `"novelty": false` is the current
  scientific result, not a pipeline failure.
- FAE pair eval:
  `.venv\Scripts\python.exe .scratch\physical-codesign-prototype\fae.py`
  — expected exit 2 and `"gate": false` until a predeclared retry wins;
  last run FAE 1/4, median ratio 2.26.
- Separate mmorch nightly was last known running:
  `schtasks /Query /TN "mmorch-nightly" /V /FO LIST`
  — wait for `Estado: Listo`, then verify `Último resultado: 0`.

## Decisions
- Prototype remains under `.scratch`; do not wire into mmorch yet.
- NumPy only; no JAX/extra dependency until a measured bottleneck requires it.
- Passive convex substrate first; XOR is out of scope because it needs active/signed or
  multistable elements.
- Readout calibration is shared by all arms and must remain explicit as a limitation.
- Simulator/checker determines fitness; LLM judgment never scores candidates.
- FAE is the named bet: software genome + hardware genome, one fitness.
  First equal-budget pair run did not beat `hw_only` (`FAE_VERDICT.md`).
  Do not wire into mmorch on that evidence.

## Open questions
- Corrected grammar vs tuned EqProp: no, not under this budget (`LEARNING_RULE_VERDICT.md`).
- Does the advantage survive more task families and true task-distribution transfer?
- Which hardware nonidealities must enter before a breadboard result is meaningful?
- Does FAE as a *pair* beat hardware-only if software mutations get more
  budget, or is EqProp-on-co-designed-graphs the ceiling? See `FAE_VERDICT.md`.

## Read first
- `HANDOFF.md`
- `IDEA.md`
- `fae.py`
- `FAE_VERDICT.md`
- `movement.py`
- `LEARNING_RULE_VERDICT.md`
- `VERDICT.md`
- `prototype.py`
- `benchmark.py`
- `learning_rules.py`
