# Hook de Stop parsea el transcript completo (2 veces) + spawn Python por turno

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: S
Eje: eficiencia
Evidence: hooks/context-block-watch.js · mmorch/context_blocks.py:40-56,69-80,93-96,35-36,137,158

`context-block-watch.js` ejecuta `execFileSync(python -m mmorch.context_blocks tick ...)`
en cada Stop; `estimate_tokens` parsea el transcript JSONL entero y sobre el umbral
`compose` lo re-parsea; tabla `context_blocks` sin índice.

**Fix:** early-exit por `os.stat().st_size/4 < threshold` antes de parsear, una sola
pasada compartida, índice (session_id, ts).

## Comments
`tick()` ahora hace `os.stat(...).st_size // 4 < thr` primero (bytes/4 es cota superior
segura del estimate real -> skip sin leer el archivo, el caso comun). Si pasa el prefiltro,
UNA sola pasada `_parse_lines()` alimenta tanto el estimate (`_chars_of_lines`) como
`_compose_from_entries` (ya no relee/reparsea). Índice `(session_id, ts)` agregado en
`_conn()`. Hook JS sin tocar (ya delega todo al proceso Python; no había redundancia de
stat del lado JS que valiera la pena mover). Self-check de `context_blocks.py` pasa.
