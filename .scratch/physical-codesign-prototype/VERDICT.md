# Verdict — topology/parameter co-design prototype

Date: 2026-09-02. Command:

```powershell
.venv\Scripts\python.exe .scratch\physical-codesign-prototype\prototype.py --seed N
```

Each arm received 320 candidate evaluations and the same initial network, task,
calibrated linear readout and executable train objective.

## Result

Held-out MSE:

- seed 7: co-design 0.001173; topology-only 0.003092; parameters-only 0.093918;
  random search 0.002878.
- seed 19: co-design 0.001561; topology-only 0.004644; parameters-only 0.037040;
  random search 0.003076.
- seed 31: co-design 0.002566; topology-only 0.004942; parameters-only 0.003320;
  random search 0.003265.

Co-design won all three seeds. Mean held-out MSE was approximately 0.00177 versus
0.00307 for random search and 0.00423 for topology-only. All winners converged in six
Newton steps with residual below 7e-12. Their ±5% conductance perturbation p95 remained
close to nominal.

## Answer

The simulator/search seam is viable enough for a second experiment. Jointly changing
connectivity and local physical parameters added value over changing either alone under
this narrow budget and task.

## What this does not prove

- Only one monotone target family and one 8-node substrate were tested.
- Held-out inputs came from the same distribution as training.
- Three seeds are directional evidence, not a statistical claim.
- The readout was recalibrated for every candidate.
- No learning-rule program was searched.
- Simulated energy decrease is not measured wall-plug energy or hardware advantage.

Next gate: repeat across multiple target families, topology sizes and distribution shifts;
then add a small typed learning-rule genome and compare joint search against fixed-rule
controls.
