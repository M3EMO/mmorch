# Hot-path re-parsea metrics.jsonl / feedback.jsonl completos por call

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: eficiencia
Evidence: mmorch/providers.py:132 · mmorch/budget.py:40 · mmorch/route.py:67,87-89 · mmorch/intuition.py:101-102 · mmorch/metrics.py:63-90 · mmorch/feedback.py:77,130-161

`providers.call()` → `budget.check()` parsea todo `logs/metrics.jsonl` (13.5k líneas,
append-only, sin rotación) antes de CADA call API con budget cap; `route()` lo re-parsea
vía `intuition.healthy()`→`error_rates()` y parsea feedback.jsonl entero vía
`calibrate_conf`. Costo O(historia completa) creciente, ×8 concurrente en fan_out.

**Fix:** cache módulo-level por (path, mtime, size) + tail-read para consumidores
ventaneados; acumulador mensual persistido para `monthly_spend`.

## Comments

Cache/tail-read en `mmorch/iohelpers.py` (`read_jsonl_cached`, `read_jsonl_tail`), por
ENCIMA de `read_jsonl_tolerant` (no la reemplaza — misma tolerancia línea-a-línea).
`metrics.read_events`/`feedback.read_outcomes` ahora cachean por (mtime_ns, size);
`error_rates(window_n=..., window_s=None)` usa `read_jsonl_tail` (solo el tail, no la
historia); `budget.monthly_spend` cachea el acumulador por `id(events)+len+month` (se
apoya en la misma invalidación de `read_events`, sin re-sumar 13.5k filas si no cambió).
Estimación ×8 fan_out: antes ~236k líneas parseadas (8 × [13.8k metrics×2 + 1.9k
feedback]); después ~17k (1 parse compartido de cada log + ≤8×200 líneas de tail) — ~93%
menos. Gates: ruff+mypy en 0 sobre `mmorch/` completo. Tests verdes: test_iohelpers,
test_budget, test_budget_policy, test_error_rates, test_feedback, test_providers,
test_cascade_bandit, test_docgen, test_innov_modules (nuevos: cache-hit mismo objeto,
invalidación por append, equivalencia con el path sin cache, en iohelpers/budget/
error_rates). `test_intuition_floor.py` tiene 2 fails preexistentes no relacionados
(`logs/health_probes.json` real con estado stale) — `intuition.py` no se tocó.
