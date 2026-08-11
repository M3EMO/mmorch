# Readers .jsonl sin tolerancia por línea: una línea corrupta tumba read_events y apaga el health floor

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: S
Eje: robustez
Evidence: mmorch/metrics.py:63-72 · mmorch/intuition.py:99-125 · mmorch/feedback.py:77 · mmorch/evolve.py:257 · mmorch/trajectory.py:128

`metrics.read_events()` parsea línea por línea sin try/continue; metrics.jsonl lo
appendean varios procesos. Una línea torn hace lanzar el reader → `intuition.healthy()`
(fail-open por diseño) deja de filtrar modelos enfermos sin señal — el guardrail que
existe porque glm-4.6 midió 34% de error. Mismo patrón en `feedback.read_outcomes`
(rompe ECE/calibración), evolve.py:257 y trajectory.py:128. El patrón correcto ya está
en el repo (arbitration.py:44-47, mcp_telemetry.py:74-75).

**Fix:** try/continue por línea en los 4 readers (copiar arbitration.py:46).

## Comments
Extraído `mmorch/iohelpers.read_jsonl_tolerant` (mismo idiom que arbitration.py) y
reusado en `metrics.read_events`, `feedback.read_outcomes`, `evolve.read_archive` y
`trajectory.load_trajectories`. `intuition.py:99-125` no se tocó (era contexto: el
`healthy()` fail-open que este fix vuelve a alimentar con señal real). Test:
`tests/test_iohelpers.py::test_read_jsonl_tolerant_skips_only_the_torn_line`.
