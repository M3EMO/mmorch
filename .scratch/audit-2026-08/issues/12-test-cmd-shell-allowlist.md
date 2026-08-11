# Enforce test_cmd propuesto por el LLM antes de shell=True (prompt-injection → RCE)

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: seguridad
Evidence: mmorch/project_integrate.py:109,150 · mmorch/project_build.py:167-170,211,84-114

El `test_cmd` per-unit lo emite el planner LLM (`_parse_worklist` lo conserva verbatim) y
llega a `subprocess.run(..., shell=True)` sin allowlist ni validación en código —
`_WORKLIST_SYS` sólo lo pide por prompt. El comentario in-code afirma que la contención
es el git worktree, pero un worktree aísla el árbol de archivos, NO la ejecución de
procesos (corre con env/red/home del usuario). Vector real de prompt-injection en `task`
→ comando arbitrario. `external_test` (usuario) es confiable; el problema es `test_cmd`
(modelo).

**Fix:** validar `test_cmd` contra allowlist/parser (prefijo de binario conocido)
rechazando metacaracteres de shell salvo el `external_test` del usuario; o
`shell=False`+`shlex.split` por default; o correr el gate en el backend docker de
sandbox.py. Mínimo: corregir el comentario que sobre-declara el worktree como contención.

## Comments
`validate_test_cmd()` nuevo en project_build.py: denylist de metacaracteres de shell
(`;&|$\`(){}<>` + redirecciones/newlines) + allowlist de binarios (pytest/python/node/npm/
go/cargo/make/...) vía `shlex.split`. Gatea DOS VECES: en `validate_worklist` (así un
test_cmd inválido dispara el re-ask normal del planner, con el error concreto) y de nuevo
en `_default_run_test` de project_integrate.py (defense-in-depth si un plan_fn inyectado
salta validate_worklist). Ejecución movida a `shell=False` + `shlex.split(test_cmd)` (ya no
hay metacaracteres que necesiten shell=True). `external_test` (el comando del usuario) NO
pasa por el allowlist — sigue `shell=True`, comentario corregido para no declarar el
worktree como contención de EJECUCIÓN (solo aísla el árbol de archivos). Rechazo = fallo
claro del gate (`test_cmd REJECTED by policy (not executed): ...`), nunca ejecución
degradada. Tests de inyección (`; rm`, `$(...)`, backticks, `|`, `&&`, redirect) vs
legítimos (`pytest -q`, `python -m pytest`, `npm test`, `go test ./...`) verificados
manualmente — todos correctos. Self-checks de project_build.py, project_integrate.py y
project_driver.py pasan; ruff+mypy en 0.
