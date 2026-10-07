# PROTOTYPE — nonlinear resistive substrate

Question: can a small energy-relaxing physical network be simulated deterministically
and improve its topology under an executable held-out-aware score?

This is throwaway code, not an mmorch module. It tests joint topology/physical-parameter
search against topology-only, parameters-only and random-search controls. A learning-rule
program is the next experiment and is unjustified unless this solver/search layer is stable.

Physical model:

```
E(v) = 1/2 sum_ij G_ij (v_i-v_j)^2
     + 1/2 sum_i leak_i v_i^2
     + 1/4 sum_i alpha_i v_i^4

tau dv_free/dt = -dE/dv_free
```

Two input voltages are clamped. Other nodes relax; one is measured as output. Positive
conductances, leak and cubic terms make the energy bounded and the equilibrium stable.
This first model is monotone: it targets analog nonlinear regression, not XOR. XOR would
require active/signed or multistable elements and is a later substrate decision.

Run:

```powershell
.venv\Scripts\python.exe .scratch\physical-codesign-prototype\prototype.py
```

The command prints the complete initial/final state and checks:

- energy never increases during relaxation;
- KCL residual converges;
- topology search improves train and held-out MSE;
- the result survives ±5% conductance perturbations;
- the same seed reproduces the same topology and metrics.
- every control receives the same number of candidate evaluations.

Promotion gate: only continue to algorithmic meta-learning if joint topology search beats
the fixed initial topology on held-out data without losing convergence or robustness.
