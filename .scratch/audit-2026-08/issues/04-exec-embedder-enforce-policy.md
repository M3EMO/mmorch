# exec_embedder: activar enforce_policy / docker al ejecutar código candidato

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: S
Eje: seguridad
Evidence: mmorch/exec_embedder.py:99,164 · mmorch/sandbox.py:10-16,45-56,117-140

`embed_exec` corre código model-generated vía `run_sandboxed` sin `enforce_policy=True`
ni `backend="docker"` — ambos existen en sandbox.py pero quedan off. El backend local,
por su propio docstring, "NO es un jail"; en Windows sin seccomp el subproceso puede
tocar red/fs fuera del cwd. Defense-in-depth faltante, no break total.

**Fix:** `enforce_policy=True` en el `run_sandboxed` de embed_exec (las sondas no
necesitan red/subprocess → sin falsos positivos) y preferir docker cuando
`docker_available()`. Documentar que en Windows sin docker la contención es best-effort.

## Comments
Bug real hallado durante el fix: `enforce_policy=True` sobre `run_sandboxed(_RUNNER, ...)`
escaneaba el harness fijo (_RUNNER), NO el código candidato no-confiable (que viaja como
`json.dumps(code)` dentro de `argv`, nunca como el `code` escrito a `_run.py`) — hubiera
sido enforcement cosmético. Fix real: `policy_violations(code)` sobre el candidato ANTES de
tocar el sandbox (embed_exec devuelve `None` si viola), más `enforce_policy=True` +
`backend="docker" if docker_available() else "local"` en la llamada a run_sandboxed
(defense-in-depth). Docstring de embed_exec documenta el best-effort en Windows sin docker.
5/5 tests de tests/test_exec_embedder.py pasan; ruff+mypy en 0.
