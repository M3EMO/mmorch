# Estado de bandits: reset silencioso, write no atómico, carrera inter-proceso

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: robustez
Evidence: mmorch/feedback.py:88-92,119-120 · mmorch/feedback.py:51-56 · scripts/nightly.py

`ThompsonBandit.__init__` resetea a `{}` sin log ante archivo corrupto (todo lo aprendido
se pierde); `update()` reescribe el JSON completo sin atomicidad; MCP server + nightly.py
escriben los mismos estados vía `record_outcome`/`intuition.record` sin lock
inter-proceso (updates concurrentes se pierden). Agravante: el bandit ya está starved
(n≤3/brazo tras 10k calls, audit 2026-07).

**Fix:** write atómico + log en corrupción; file-lock o migrar el estado a SQLite
(patrón workflow_store.py).

## Comments
`ThompsonBandit.__init__` usa `iohelpers.load_json_tolerant` (log fuerte, no reset
mudo) y `update()` usa `atomic_write_json` (elimina la corrupción por crash mid-write,
que era la causa citada). NO agregué file-lock inter-proceso ni migré a SQLite (fuera de
scope explícito: "fix mínimo"); el lost-update entre MCP server y nightly.py bajo updates
verdaderamente concurrentes sigue siendo posible — documentado, no resuelto acá.
`bandit_sig.json` (intuition.py) usa el mismo `ThompsonBandit`, así que hereda el fix
sin tocar intuition.py.
