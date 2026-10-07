# Verdict — typed learning-rule search (corrected)

Date: 2026-09-03. Command:

```powershell
.venv\Scripts\python.exe .scratch\physical-codesign-prototype\learning_rules.py
```

Learning rate is in the genome. Search starts from random primitives (no
seeded `sign_eqprop`). Each named control received the same number of
meta-evaluations as the search (`16`). Algebraic duplicates are canonicalized.
Mean-OOD ties use abs `1e-12` / rel `1e-8`.

## Result

Mean held-out OOD MSE (12 cases: 3 families × 2 sizes × 2 seeds):

- tuned `sign_eqprop`: `0.01476` (unique best)
- tuned `canonical_eqprop`: `0.02099`
- evolved `clip(mul(free_energy, free_energy))` at lr `0.109`: `0.02771`
- no learning: `0.02858`

`novelty`: false. The evolved rule is not an algebraic control, but it loses
to both tuned EqProp variants. Pipeline gate (equal meta budget) holds.
Exit 0 after the pipeline check; novelty is a field, not a fake win count.

## Answer

Fair search did not discover a better local rule than tuned normalized
EqProp under this grammar, budget and substrate. Do not claim a novel
learning rule. Hardware-half search may still use `sign_eqprop` as the
software control.
