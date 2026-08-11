# projects.json contaminado: paths temp/home/Desktop registrados, sin GC

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: S
Eje: coherencia
Evidence: scripts/autoregister_project.py:13 · mmorch/projects.py:57-65

Path pytest temporal muerto, home dir completo ("map12") y Desktop entero ("Claude")
registrados como proyectos job-controlables. Causa: `_SKIP` solo excluye orchestration en
SessionStart; sin GC (resolve() falla sobre muertas pero nadie poda).

**Fix:** filtro temp/home en `_SKIP` + `prune()` dry-run-default en projects.py.

## Comments
`_SKIP` en autoregister_project.py ahora excluye home dir y Desktop root (exact match) más
cualquier path bajo `tempfile.gettempdir()` (prefijo — cubre tmpdirs de pytest antes de que
queden muertos). `prune(store=None, dry_run=True)` nuevo en projects.py: reporta entradas
con path no-directorio, solo escribe si `dry_run=False` (higiene explícita, no side-effect
de leer el registro). No se tocó projects.json existente (datos de runtime, no código; el
operador corre `prune(dry_run=False)` cuando quiera aplicar). 4 tests nuevos en
tests/test_project_aware.py (prune dry-run/apply/noop + filtro home/Desktop/temp/proyecto
real) — todos pasan; ruff+mypy en 0.
