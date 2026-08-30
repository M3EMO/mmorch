---
source: decision-systems-majority-voting-requiere-independencia-pane.md
lexicon: v2
ratio: 0.468
fidelity: 0.833
derived: true
---
Decision systems: majority voting requires independence—panel same-model no decorrelates.
2026-08-29. [research, orchestration, decision-theory, ensemble, verification, evolve, llm-as-judge]. applied. 0.9.
Sources: wikipedia.org/wiki/Condorcet%27s_jury_theorem, arxiv.org/html/2605.29800, arxiv.org/abs/2404.18796, arxiv.org/html/2607.08065.

Q: Fix LLM verifier strict (SAME model 3x, majority)? (eval `mmorch/evolve.py::_ensemble_check`, pre-PR gate)

General: Condorcet Jury Theorem.
Classic: panel > chance (p>0.5) converges w/ large N—**independence assumption**. Decay w/ correlation. Wisdom-of-crowds needs diversity+independence+decentralization.

AI agents: LLM-as-judge panels.
- PoLL (Verga et al. 2024, arXiv:2404.18796): SMALL models, DIFFERENT families > single large judge, 7x cheaper—advantage from "disjoint model families".
- "Nine Judges, Two Effective Votes" (arXiv:2605.29800, 2026): 9 judges, 7 families ≈ best individual. **n_eff** (Kish: `n_eff = k / [1 + (k-1)·φ̄]`)—φ̄=0.391, n_eff≈2.18. SAME model (φ≈1) gives **n_eff≈1**: zero gain.
- Self-consistency (Wang et al. 2022) works for reasoning (multiple inference PATHS). Not for POLICY/CAUTION JUDGMENT: bias reproduced. Audit (arXiv:2607.08065): "voting reduces variance, not bias"—false confidence w/ systematic bias.

Verdict.
**Incorrect.** Repeating same verifier N times ≠ valid ensemble. Own case: 5 `server.py` branches rejected by 1 verifier (Google) were security fixes; 3x same-family panel would reproduce rejection.

Fix applied (hybrid).
`mmorch/evolve.py::_ensemble_check` (red zone): DETERMINISTIC check (`_diff_only_adds_guards`, 0 bias/cost) auto-passes common pattern—diff doesn't lose content (wrap line `try/except`) & adds guard (`return ... status_code=40x` / `raise`). Backtest (5 branches): 1/5 passes deterministically (rest modify lines beyond wrapping→SINGLE honest LLM call).

Next step.
Decorrelates: SECOND verifier family (Kimi inactive). PoLL paper & [[llm-as-jury-ensemble-y-errores-correlacionados.md]] requested. Same-model panel NOT substitute.
