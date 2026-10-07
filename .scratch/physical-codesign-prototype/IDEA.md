# Idea — FAE (fullstack evolutionary algorithm pair)

Status: prototyped and gated. Pair lost (`FAE_VERDICT.md`: 1/4 wins, median
ratio 2.26). Software novelty false (`LEARNING_RULE_VERDICT.md`). Hardware
movement seed 7 still passed. Not wall-plug joules. Fitness is a checker;
LLM judgment does not score candidates.

**FAE** is one loop, not two products: an evolutionary software genome and an
evolutionary hardware genome scored by the same frozen fitness. Isolated
search already lost (`VERDICT.md`: co-design beat topology-only,
parameters-only, and random). Evolving software against a frozen ISA (H100)
or hardware without the task is not this bet.

```
min  MSE_task + λ · (Newton_steps · edges · batch)
```

| Half | Genome | Code today | State |
|---|---|---|---|
| Hardware | edges, G, leak, alpha | `prototype.py`, `movement.py` | seed 7: hops `18240 → 14400`, MSE `0.00307 vs 0.00335` |
| Software | local update rule + learning rate | `learning_rules.py` | fair search; tuned `sign_eqprop` still wins; novelty false |
| Pair | both, one objective | `fae.py` | gate false: `hw_only` won 3/4 |

Software without hardware search ports to NVIDIA. Hardware without the
task is a pretty graph. The pair is the IP: the program *is* the circuit,
and data does not leave the substrate except one ADC per sample.

Echo readout and a photonic GPU were dropped: they are not a realistic
investment from this prototype. Photonic I/O is already a funded product
(Passage, CPO). Extra analog samples are more ADCs, and an ADC is data
movement. The edge against shipping GPUs is not more FLOPS.

## Claim to design against

A GPU already multiplies enough. Decode sits near 1 FLOP/byte while the
chip’s ridge is hundreds to thousands of FLOPs/byte. Energy is

```
E = e_op * N_op + e_byte * N_byte + e_net * N_hop
```

with `e_byte >> e_op`. The product is: keep weights and activations where
they compute, and emit a small result. The prototype’s conductance matrix
already does that if we do not stream `G` or `v` through a memory bus.

## Design

Same 8-node convex resistor network as `prototype.py`. Two search arms,
equal candidate evaluations, same seed family:

| Arm | Objective | What it optimizes |
|---|---|---|
| `mse_only` | `MSE + 2e-5 * edges` | accuracy, weak sparsity |
| `movement` | `MSE + 2e-5 * edges * Newton_steps` | accuracy plus *internal traffic* |

Internal traffic proxy (analog message-passing, not the dense NumPy
Newton tax):

```
hops = Newton_steps * edges * batch
```

Each existing edge carries a local voltage difference once per settle
step. Stationary `G` costs zero HBM loads. The only billed digital
conversion is one output ADC per sample.

Counterfactual, not a search arm: if `G` were loaded from HBM every
Newton step, bytes scale as `steps * n² * batch`. That is the GPU-style
cost of the *same* math. Report the ratio; do not gate on it (the cost
model would make analog win by construction).

Dropped:

- Echo / multi-sample readout — more ADCs, so more `N_byte`.
- Photonic multidirectional `S` — a different chip company; this effort
  does not have a foundry path.

## Evaluation

Command:

```powershell
.venv\Scripts\python.exe .scratch\physical-codesign-prototype\movement.py
```

Predeclared gate (all must hold):

1. `movement` hops on held-out < `mse_only` hops.
2. `movement` held-out MSE ≤ 2× `mse_only` held-out MSE.
3. `movement` held-out MSE < initial network held-out MSE.
4. Same seed reproduces the JSON of metrics.

Failing the gate is an honest result: locality search did not buy
movement without wrecking the task. Do not narrate it as a win.

What a pass still does not prove: joules, LLM decode, XOR, or a chip.
It only shows that searching the graph for fewer edge-steps, on this
substrate and task, reduces the movement proxy at bounded MSE.

## Pointers

- Pair evaluator: `fae.py`, `FAE_VERDICT.md`
- Movement evaluator: `movement.py`
- Solver / co-design: `prototype.py`, `VERDICT.md`
- Software half: `learning_rules.py`, `LEARNING_RULE_VERDICT.md`
