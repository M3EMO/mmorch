# Hot-path re-parsea metrics.jsonl / feedback.jsonl completos por call

Type: task
Status: open
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
