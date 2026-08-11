# Handoff — auditoría EJE=seguridad — 2026-08-10

## Qué se cubrió

- **Gates estáticos** (via `.venv`): `ruff check .` = 0, `mypy mmorch --ignore-missing-imports` = 0.
  Sin regresión. (Nota packaging: `mypy .` a secas rompe por `flywheel/oracle_dataset.py` sin
  `__init__.py` — fuera del gate enforced, no es hallazgo.)
- **Checklist completo (3 ejes):**
  - *Aislamiento código LLM:* grep exhaustivo de `exec`/`eval`/`__import__`/`importlib`/`compile`.
    Único `exec()` real de código de modelo = `exec_embedder.py` (→ F-2). El resto: regex-strings,
    imports de stdlib/módulos internos, plugin_worker (dirs locales confiables). Sandbox honesto sobre
    sus límites.
  - *Secrets:* keys sólo de env por nombre de var (nunca valor); `.env` git-ignored; sin keys en
    logs/emit/errores; redacción de transcript en sessions.py + feedback_trace.py. Único match de key
    en logs = ejemplo de yt-dlp en un sample de dataset (no del usuario).
  - *Prompt injection:* outputs de modelo se parsean como DATOS acotados (route/classify/bucketrank
    regex escalar; adversarial_verify con frontera "ARTIFACT TO REFUTE"). El ÚNICO punto donde output
    de modelo se convierte en acción es el `test_cmd` del planner → shell (F-1).
- **Profundo:** server MCP (auth token en todos los endpoints, bind localhost), tools (mcp_server.py,
  vault_write), memoria/estado (memory.py SQL parametrizado; vault.py; workflow_store), engine
  project_* (traversal de escritura guardada por _safe_target).
- **Liviano:** hooks globales (execFileSync sin shell; never-edit-guard fail-open/closed correcto),
  skills (sin ejecución de código de modelo).
- **Verificación:** los 2 hallazgos pasaron por `mmorch_adversarial_verify` (DeepSeek→Gemini). El
  verificador concedió los hechos técnicos y sólo discutió severidad → sobreviven; ajusté F-1 a
  IMPORTANTE recogiendo su punto (opt-in).

## Qué quedó fuera / límites

- No se ejecutó nada en runtime (auditoría read-only); F-1 no se explotó, se trazó por código.
- No se auditó el contenido del vault, backups, ni `.scratch/` (excluido por alcance).
- `bandit`/escáner SAST dedicado no está instalado en el venv; me apoyé en ruff (select F/E9/B/PLE,
  que NO incluye reglas S de seguridad) + greps dirigidos + lectura. Un pase de `bandit -r mmorch`
  sería complementario si se quiere red-team más profundo del subprocess/shell surface.
- No se revisó a fondo `pty_session.py`/`server_pty.py` (ConPTY) más allá de confirmar que abre un
  shell interactivo detrás del token del server — potencial superficie pero gated por auth.

## Señales para otros ejes

- **Robustez:** F-1 y F-2 tocan `subprocess`/`shell=True` — el eje robustez debería mirar timeouts,
  manejo de TIMEOUT y degradación de esos mismos call-sites (project_integrate, autoresearch:110,
  project_loop:39). Hay MUCHOS `shell=True` (autoresearch, project_loop, project_integrate) que además
  de seguridad son superficie de robustez.
- **Costo/API:** no evaluado aquí, pero `providers.call` tiene BudgetKeeper + error_class logging —
  buen gancho para el eje de costo.
- **Duplicación:** varios helpers `_run_cmd`/`_git`/subprocess wrappers casi idénticos entre
  project_loop.py, autoresearch.py, evolve.py, sync.py, worktree_driver.py — candidato para el eje de
  dup estructural.
- **pty/ConPTY** queda como superficie para quien profundice el eje server.
