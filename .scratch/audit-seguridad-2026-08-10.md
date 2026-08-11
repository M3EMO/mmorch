# Auditoría EJE=seguridad — mmorch — 2026-08-10

Read-only. Alcance profundo: server MCP, tools, memoria/estado (SQLite, vault). Alcance
liviano: hooks globales (`~/.claude/hooks`), skills. Excluido: backups, `.scratch/`, vault, docs.

## Gates estáticos (baseline, gratis)

- `ruff check .` (via `.venv`) → **All checks passed** (0). Sin regresión.
- `mypy mmorch --ignore-missing-imports` → **Success: no issues found in 100 source files** (0).
  (Correr `mypy .` a secas falla por `flywheel/oracle_dataset.py` sin `__init__.py` — es ruido de
  packaging fuera del gate enforced `mmorch`, no un hallazgo de seguridad.)

Ambos gates en 0: sin hallazgos de nivel gate.

## Hallazgos sobrevivientes

Los 2 candidatos pasaron por `mmorch_adversarial_verify` (DeepSeek→Gemini, refutar por default).
En ambos el verificador devolvió `passed=false` PERO sus refutaciones **conceden todos los hechos
técnicos del rubric** y sólo discuten el encuadre de severidad (F-1: "es opt-in / usuario
autorizado"; F-2: "es peor que hardening-gap, es fallo de implementación"). Es decir: la sustancia
técnica sobrevive; ajusté la severidad a la baja en F-1 recogiendo el punto válido del verificador.

---

### F-1 — IMPORTANTE — Comando propuesto por el modelo ejecutado con `shell=True` (prompt-injection → ejecución de comando)

**Evidencia:**
- `mmorch/project_integrate.py:109` — `subprocess.run(test_cmd, cwd=repo, shell=True, ...)` (per-unit).
- `mmorch/project_integrate.py:150` — `subprocess.run(external_test, cwd=repo, shell=True, ...)`.
- `mmorch/project_build.py:211` — `_parse_worklist` conserva `"test_cmd": u.get("test_cmd")` **verbatim** del JSON del planner (DeepSeek).
- `mmorch/project_build.py:167-170` — `_WORKLIST_SYS` sólo **instruye** ("test_cmd must be an existing/user-provided command or null"). No hay allowlist ni validación en código (`validate_worklist`, `mmorch/project_build.py:84-114`, no chequea `test_cmd`).

**Análisis:** El `external_test` lo provee el usuario (confiable). Pero el `test_cmd` per-unit lo
**inventa el planner LLM** y llega intacto a `shell=True` en el HOST. `_run_project_build_job`
(`mmorch/server_engine.py:295-355`) corre `build_project` dentro de `open_worktree(...)`, y el
comentario in-code (project_integrate.py:106-107, 147-148) afirma que la contención es "the isolated
git worktree (exec_policy)". **Esa afirmación sobre-alcanza:** un git worktree aísla el ÁRBOL DE
ARCHIVOS, no la ejecución de procesos — el comando corre con el entorno completo del usuario, red y
home. No hay container/seccomp/allowlist sobre `test_cmd`. (La traversal de *escritura* sí está bien
guardada aparte por `_safe_target`, project_integrate.py:39-50.) La cadena de riesgo real: un `task`
con prompt-injection puede orientar al planner a emitir un `test_cmd` malicioso, que se ejecuta.

**Por qué IMPORTANTE y no BLOCKER:** es un job opt-in que el usuario lanza (no un camino por
default), y el disparador principal es un comando alucinado/inducido, no un atacante externo directo.
Aun así el vector es real y la mitigación citada (worktree) no aplica a ejecución de comando.

**Fix propuesto (descripción, sin diff):** hacer cumplir en código lo que el prompt ya promete —
validar `test_cmd` contra una allowlist/parser (prefijo de binario conocido: `pytest`, `python -m`,
`npm test`, `node`, el propio `external_test` del usuario…), rechazando metacaracteres de shell
peligrosos (`;`, `|`, `&&`, backticks, `$(`, redirecciones) cuando el comando NO es el `external_test`
provisto por el usuario. Alternativa/complemento: correr `test_cmd` con `shell=False` + `shlex.split`
salvo cuando el usuario marque explícitamente un comando compuesto, o ejecutar el gate en el backend
docker de `sandbox.py` (`--network none`) cuando esté disponible. Como mínimo, corregir el comentario
que sobre-declara el worktree como contención de ejecución.

---

### F-2 — IMPORTANTE — `exec_embedder` ejecuta código candidato sin el escaneo de policy disponible ni jaula docker

**Evidencia:**
- `mmorch/exec_embedder.py:164` — `run_sandboxed(_RUNNER, timeout=_TIMEOUT, argv=[...])` — sin
  `enforce_policy=True`, sin `backend="docker"` (ambos default a off/local).
- `mmorch/exec_embedder.py:99` (dentro de `_RUNNER`) — `exec(SRC, ns)` ejecuta el código candidato.
- `mmorch/sandbox.py:10-16` — el propio docstring: local es "razonablemente aislado, **NO un jail**";
  en Windows "no hay seccomp/namespaces → el subproceso PUEDE tocar la red y el filesystem fuera del cwd".
- `mmorch/sandbox.py:45-56, 117-140` — `policy_violations`/`_POLICY_DENY` (escaneo estático pre-exec:
  socket/subprocess/os.system/fs-write) y backend docker (`--network none --read-only --cap-drop ALL`)
  **existen** pero son opt-in y esta ruta no usa ninguno.

**Análisis:** El `code` que `embed_exec` corre es, en el flywheel, generado por modelos. Se ejecuta en
subproceso con entorno mínimo (sin secrets, `sandbox.py:96-100`) y kill-timeout — cumple la invariante
de CLAUDE.md "aislar código LLM-generado en subprocess" en la LETRA. Pero en Windows el backend local
no es contención real contra un snippet hostil, y el screen estático `enforce_policy` (barato, ya
implementado) se saltea en el camino de embedding. Defense-in-depth faltante, no un break total: el
subprocess+env-mínimo+timeout es la barra documentada. (El verificador argumentó que es "fallo de
implementación" y no mero hardening-gap; lo dejo en IMPORTANTE.)

**Fix propuesto:** en `embed_exec`, pasar `enforce_policy=True` al `run_sandboxed` (el runner de
sondas es código conocido; el `SRC` candidato es lo peligroso, y las sondas no necesitan red/subprocess
→ el denylist no debería dar falsos positivos legítimos), y preferir `backend="docker"` cuando
`docker_available()` en la ruta de fingerprinting de código no confiable. Documentar que en Windows sin
docker la contención es best-effort.

---

## Controles positivos observados (no son hallazgos)

- **Secrets:** las keys se cargan sólo de env (`mmorch/config.py` guarda `api_key_env`, el NOMBRE de
  la var, nunca el valor; `providers.py:92-100`). `.env` está git-ignored (verificado con
  `git check-ignore`). `MissingKeyError` (`providers.py:93-97`) reporta el nombre de la var, no la
  clave. No hay keys en logs de código/emit/print (grep dirigido, negativo). El único match `AIzaSy…`
  en `logs/*_dataset.jsonl` es una API key de ejemplo de yt-dlp DENTRO de un sample de dataset de
  código (contexto "Downloading video webpage"), NO un secret del usuario — no coincide con ninguna
  clave de `.env` (comparación programática, cero overlap).
- **Redacción de transcript:** `mmorch/sessions.py:118-131` y `mmorch/feedback_trace.py:27-31` scrubean
  PEM/JWT/AWS/api-keys/emails/home-paths antes de que el material viaje a memoria/vault.
- **Server MCP:** auth por token en TODOS los endpoints (`_token_ok`, `server_core.py:19-24`), bind
  default `127.0.0.1` (`server.py:9-18`). Modo dev sin token documentado y bindeado a localhost.
- **Aislamiento de plugins:** `mmorch/plugins.py:70-110` — subprocess worker con gate de capabilities
  de dos capas + kill-timer; los plugins son dirs locales confiables, no output de modelo.
- **Zona-roja evolve:** `mmorch/evolve.py:270-305` — screening de contenido (dinero/borrado/SO/red/
  claves) + path denylist + goal_guard tamper-halt antes de auto-aplicar. Nunca auto-aplica rojo.
- **Hooks globales (pasada liviana):** `context-block-*.js` usan `execFileSync` con args en array (sin
  shell, sin interpolación) → sin inyección. `never-edit-guard.js` falla-open ante error de infra,
  falla-closed sólo ante match explícito, y se auto-protege (`.env`, `never-edit.txt`, `goal.hash`).
- **Parsing de output de modelo como DATOS:** `route.py`/`classify.py`/`bucketrank.py` extraen escalares
  acotados por regex (CONFIDENCE/CLASS/TIER); `patterns.adversarial_verify` interpola el artefacto bajo
  una frontera explícita ("ARTIFACT TO REFUTE"). Ningún output de modelo selecciona/ejecuta tools
  directamente. `claude_exec.run_claude` corre con `--permission-mode plan` (read-only) por default.

## Apéndice — Descartados

- *SQL injection en `memory.py`* — `con.execute(f"ALTER TABLE semantic ADD COLUMN {ddl}")` (l.126) y
  `f"... WHERE id IN ({ph})"` (l.197): el `ddl` viene de una tupla literal hardcodeada; `ph` son sólo
  placeholders `?` y los ids van parametrizados. No hay input de usuario/modelo en el SQL. Descartado.
- *`exec_embedder` viola "aislar código LLM en subprocess"* — SÍ corre en subprocess (letra cumplida);
  el gap real es enforce_policy off, capturado en F-2. No es un segundo hallazgo.
- *`evolve.py` menciona `os.system`/`eval` en strings* — son la regex de firmas zona-roja y ejemplos de
  test, no ejecución. El propio `red_content_hits(baseline=...)` corrige el auto-lock. Descartado.
- *Secret `AIzaSy…` en logs* — API key de ejemplo de yt-dlp dentro de un sample de dataset, no un
  secret del usuario; cero overlap con `.env`. Descartado.
