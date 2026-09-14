# supervision.md — driver_py


## candidato 14:07:04
revision de spec (Claude, rc=0): Hice 0 preguntas: la spec ya cubre el barrido completo, sin ambigüedad que cambie código.

## candidato 14:21:22
ESCALATE_HUMAN: aceptacion verde pero suite total con regresion
fallos nuevos: ['tests/test_executor_seam.py::test_coder_loop_escalates_through_fake_executor', 'tests/test_executor_seam.py::test_coder_loop_fake_executor_failure_propagates', 'tests/test_project_loop.py::test_escalates_to_claude_after_K', 'tests/test_project_loop.py::test_mmorch_solves_after_correction', 'tests/test_project_loop.py::test_no_escalation_returns_failure', 'tests/test_project_loop.py::test_no_test_cmd_breaks_unverified']

## candidato 14:48:24
nivel 3 Claude (rc=0): Corrijo las oraciones largas.

Test de aceptación pasa. Los tests rotos por el cambio anterior también vuelven a pasar.

**Qué cambié**, en `mmorch/project_loop.py`:
- Reescribí `_codegraph_context`. Ya no indexa sin opt-in explícito.
- `MMORCH_CODEGRAPH_AUTOINDEX` cambia su default a `'0'`.
- Con opt-in, tampoco indexa si el repo está bajo `tempfile.gettempdir()`.
- `sync` sobre `.codegraph` existente sigue igual que antes.
- Actualicé el docstring de la función.
- Restauré el resto del módulo. Una edición previa lo había reescrito por error. Esa edición perdió el docstring del módulo y funciones como `_checkpoint`, `ProjectResult` y `run_project_task`.

## candidato 14:54:05
revision del diff (Claude, rc=0): BLOCK: R1 llama `shutil.which("codegraph")` antes de chequear el opt-in. Esto viola `spec.md`. El test falla en rojo y confirma el defecto.

## candidato 14:57:21
reasoner 1: {"files": ["mmorch/project_loop.py"], "instructions": {"mmorch/project_loop.py": "Reordenar _codegraph_context para cumplir R1: hoy resuelve `bin_` con `os.environ.get(\"CODEGRAPH_BIN\") or shutil.which(\"codegraph\")` ANTES de chequear el opt-in, por lo que sin `.codegraph` y sin opt-in igual invoca shutil.which. Mover el bloque de resolucion de binario (`bin_ = ... ; if not bin_: return \"\"`) a DESPUES de los chequeos de hermeticidad. El orden debe ser: (1) calcular `cgdir = os.path.join(repo, \".codegraph\")` y `has_index = os.path.isdir(cgdir)`; (2) si `not has_index`, chequear opt-in `if os.environ.get(\"MMORCH_CODEGRAPH_AUTOINDEX\", \"0\") != \"1\": return \"\"`; (3) si hay opt-in, comparar paths resueltos contra `tempfile.gettempdir()` y devolver \"\" si `repo_resolved == temp_dir or temp_dir in repo_resolved.parents`; (4) recien ahora resolver `bin_ = os.environ.get(\"CODEGRAPH_BIN\") or shutil.which(\"codegraph\")` y devolver \"\" si es falsy; (5) el resto (sync/init/index/context) queda igual. Ajustar el docstring de la funcion para que describa explicitamente este orden: sin `.codegraph` y sin MMORCH_CODEGRAPH_AUTOINDEX=1 (default '0') devuelve '' sin resolver el binario ni tocar el repo; con opt-in, repos bajo tempfile.gettempdir() tampoco se indexan; con `.codegraph` presente el binario se resuelve y `sync` corre como siempre."}}

## candidato 14:57:58
reasoner 2: {"files": ["mmorch/project_loop.py"], "instructions": {"mmorch/project_loop.py": "En `_codegraph_context`, el orden actual resuelve el binario antes de chequear el opt-in: `bin_ = os.environ.get(\"CODEGRAPH_BIN\") or shutil.which(\"codegraph\")` ocurre ANTES de calcular `cgdir`/`has_index` y del early-return por falta de opt-in, por lo que `shutil.which` se invoca aunque no haya `.codegraph` ni `MMORCH_CODEGRAPH_AUTOINDEX=1` (viola R1). Reordena asi: primero calcula `cgdir = os.path.join(repo, \".codegraph\")` y `has_index = os.path.isdir(cgdir)`; si `not has_index`, checa `if os.environ.get(\"MMORCH_CODEGRAPH_AUTOINDEX\", \"0\") != \"1\": return \"\"` y luego el guard de tempdir (`Path(tempfile.gettempdir()).resolve()` vs `Path(repo).resolve()`, return \"\" si esta bajo temp); SOLO DESPUES de esas decisiones resuelve `bin_ = os.environ.get(\"CODEGRAPH_BIN\") or shutil.which(\"codegraph\")` y `if not bin_: return \"\"`. Asi `shutil.which` no se llama sin opt-in (R1), con opt-in y repo temporal tampoco se indexa (R2), y con `.codegraph` presente el `sync` y el resto siguen igual (R3). Actualiza el docstring de la funcion para reflejar que el binario se resuelve despues del gate de opt-in/tempdir."}}

## candidato 14:59:46
nivel 3 Claude (rc=0): Cambié `mmorch/project_loop.py`. Moví la resolución de `bin_` después del chequeo de opt-in. Así sin opt-in no se invoca `shutil.which`.
