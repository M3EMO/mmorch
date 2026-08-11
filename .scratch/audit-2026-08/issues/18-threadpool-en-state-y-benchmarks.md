# state_snapshot/benchmarks_handler bloquean el event loop con parses sync

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: S
Eje: eficiencia
Evidence: mmorch/server.py:42-61,290-327,276,337

Handlers async con 2-4 parses completos de metrics.jsonl inline → SSE /events y requests
concurrentes esperan. El archivo ya usa `run_in_threadpool` (:276, :337).

**Fix:** fondo = ticket 13; complemento: envolver estos handlers en run_in_threadpool
(mitiga, no elimina contención de GIL).

## Comments
`state_snapshot` y `benchmarks_handler` factoreados a `_state_snapshot_sync()` /
`_benchmarks_sync()` + `await run_in_threadpool(...)`, mismo patrón que `chat_handler`
(:276) y `minds_handler` (:337). El cache real de metrics.jsonl (ticket 13) sigue
pendiente y no es mío — esto solo saca el parse sync del event loop. `import mmorch.server`
+ ruff/mypy en 0.
