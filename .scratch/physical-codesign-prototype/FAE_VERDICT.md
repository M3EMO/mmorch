# Verdict — FAE pair (software genome × hardware genome)

Date: 2026-09-03. Command:

```powershell
.venv\Scripts\python.exe .scratch\physical-codesign-prototype\fae.py
```

Four arms, 24 evaluations each, same initial net per case, objective
`OOD MSE + 2e-5 * edges * Newton_steps`. FAE and software-only start from
a random rule (not seeded EqProp). Hardware-only freezes `sign_eqprop` and
mutates topology/parameters plus learning rate.

## Result

4 cases (tanh_mix, nested_tanh × seeds 7, 19), 6 nodes.

- FAE objective wins: 1/4 (win rate 0.25)
- median ratio vs best control: `2.26` (p90 `4.19`)
- predeclared gate (`wins >= 50%` and median ratio `< 0.95`): **false**
- winner in 3/4 cases: `hw_only`

Software-only was the weakest arm. FAE did not recover EqProp in 24
mutations often enough to beat a control that starts there.

## Answer

Under this budget the pair is not better than evolving hardware with a
fixed tuned-class rule. The named FAE bet is not supported yet. Next
levers, if any: more evaluations on the software mutations, or FAE that
*may* keep EqProp as a primitive rather than forbidding it — that would
be a different experiment and must be predeclared.

This is not joules, not a chip, and not a reason to wire FAE into mmorch.
