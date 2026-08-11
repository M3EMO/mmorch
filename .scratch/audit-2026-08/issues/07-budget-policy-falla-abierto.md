# budget_policy.load() falla abierto y en silencio: JSON corrupto desactiva los límites de gasto

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: robustez
Evidence: mmorch/budget_policy.py:22-26,30 · mmorch/server_core.py:27-36

`load()` traga cualquier excepción devolviendo `[]` sin log — no distingue "no hay
políticas" de "las políticas no se pudieron leer". `blocking_incident()` (cableado al 402
de creación de jobs) con `[]` no bloquea nada: un `budget_policies.json` truncado (que el
propio `save()` no-atómico puede producir en crash mid-write) anula todos los hard-stops
de gasto sin señal.

**Fix:** distinguir no-existe (`[]` legítimo) de no-parsea (log fuerte y/o incidente hard
conservador) + `save()` atómico (tmp + `os.replace`).

## Comments
`load(strict=True)` ahora propaga `PolicyLoadError` en JSON corrupto (con log fuerte);
`blocking_incident()` la captura y devuelve un incidente `hard` sintético
(scope `"*"`) en vez de fallar abierto — bloquea job-creation hasta que se repare el
archivo. `load()` (no-strict, default) sigue devolviendo `[]` para no romper callers
existentes. `save()` ahora usa `iohelpers.atomic_write_json`. Tests:
`tests/test_budget_policy.py`.
