# Tracker entries — EJE=seguridad (2026-08-10)

Para que el orquestador mergee al tracker local (`.scratch/<feature-slug>/issues/NN-<slug>.md`).
Feature-slug sugerido: `audit-seguridad`. Rankeados: severidad primero, luego menor esfuerzo.

---

## 01 — Enforce test_cmd propuesto por el LLM antes de shell=True (prompt-injection → RCE)

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/project_integrate.py:109,150 · mmorch/project_build.py:167-170,211 · validate_worklist project_build.py:84-114

### Body

El `test_cmd` per-unit lo emite el planner LLM (`_parse_worklist` lo conserva verbatim,
project_build.py:211) y llega a `subprocess.run(..., shell=True)` (project_integrate.py:109) sin
allowlist ni validación en código — `_WORKLIST_SYS` sólo lo pide por prompt. El comentario in-code
afirma que la contención es el git worktree, pero un worktree aísla el árbol de archivos, NO la
ejecución de procesos (corre con env/red/home del usuario). Vector real de prompt-injection en `task`
→ comando arbitrario. `external_test` (usuario) es confiable; el problema es `test_cmd` (modelo).

**Fix:** validar `test_cmd` contra allowlist/parser (prefijo de binario conocido) rechazando
metacaracteres de shell (`;|&$()` backticks/redir) salvo el `external_test` provisto por el usuario;
o `shell=False`+`shlex.split` por default; o correr el gate en el backend docker de sandbox.py.
Mínimo: corregir el comentario que sobre-declara el worktree como contención de ejecución.

---

## 02 — exec_embedder: activar enforce_policy / docker al ejecutar código candidato

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/exec_embedder.py:99,164 · mmorch/sandbox.py:10-16,45-56,117-140

### Body

`embed_exec` corre código (model-generated en el flywheel) vía `run_sandboxed` sin `enforce_policy=True`
ni `backend="docker"` — ambos existen en sandbox.py pero quedan off. El backend local, por su propio
docstring, "NO es un jail"; en Windows sin seccomp el subproceso puede tocar red/fs fuera del cwd. Se
cumple la letra de "aislar en subprocess" (env mínimo + timeout) pero se saltea el screen estático
disponible. Defense-in-depth, no break total.

**Fix:** pasar `enforce_policy=True` en el `run_sandboxed` de embed_exec (las sondas no necesitan
red/subprocess → sin falsos positivos legítimos) y preferir docker cuando `docker_available()`.
Documentar que en Windows sin docker la contención es best-effort.
