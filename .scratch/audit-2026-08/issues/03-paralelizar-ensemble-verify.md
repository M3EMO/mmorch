# ensemble_verify corre K verificadores API en serie

Type: task
Status: open
Severity: IMPORTANTE
Effort: S
Eje: eficiencia
Evidence: mmorch/ensemble.py:56-58 · mmorch/patterns.py:67

ensemble.py:56-58: list comprehension secuencial de `adversarial_verify` (5-30 s c/u).
Calls independientes; el repo ya paraleliza 8 en `fan_out` (patterns.py:67).

**Fix:** ThreadPoolExecutor preservando orden de verdicts por índice.

## Comments
ThreadPoolExecutor(max_workers=len(verifier_models)) + `ex.map` (heredado, ya preservaba
orden por índice). Agregado `tests/test_ensemble_parallel.py`: orden estable bajo latencia
desigual, equivalencia de resultado vs. la agregación serial, y sanity de wall-clock
paralelo. ruff+mypy en 0.
