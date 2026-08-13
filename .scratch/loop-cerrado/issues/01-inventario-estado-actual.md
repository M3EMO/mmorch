# 01 — Inventario del estado actual del loop

Type: research
Status: resolved

## Question

Levantar los HECHOS que todas las decisiones siguientes necesitan, con file:line:

1. Qué emiten hoy y en qué formato: `nudge.py` (tick/SessionStart), hook de
   session_ingest (SessionEnd), context-block (Stop), nightly evolve
   (`evolve.py` / tarea mmorch-nightly), resumen matutino 09:10.
2. `record_outcome` / feedback: firma exacta, quién lo llama hoy, qué consume
   el bandit (`ThompsonBandit`) y qué significa "brazo" actualmente.
3. Registry de proyectos (`projects.py`): qué campos tiene un proyecto
   autoregistrado, cuántos hay, cuáles tienen `.codegraph/`.
4. Vault: cuántas notas, qué frontmatter llevan (tags de proyecto ya
   obligatorios), cómo funciona recall/ranking hoy.
5. Roadmap actual de innovate: dónde vive, último update, cómo lo consume
   `mmorch_innovate`; estado de evolve_open_prs.
6. Qué hooks globales existen en `~/.claude/settings.json` y qué presupuesto de
   latencia toleran (para saber dónde puede colgarse la inyección de propuesta).

## Answer

### 1. Qué emite cada pieza hoy

- **`mmorch/nudge.py`** — NO es un hook: es una función de librería. `tick()` (nudge.py:29-65) la llaman `rubric_loop.py:296-299` y `code_loop.py:114-116` al cerrar un loop. Cada 10 closes (`_EVERY=10`, nudge.py:21) corre `memory.consolidate` + `distill_backlog(limit=5)` y devuelve un dict `{closes, nudged, report}` al caller (no emite nada a la sesión). Estado en `logs/nudge.json` con guard monotónico de `distill_upto` (nudge.py:57-64).
- **SessionStart (hook global)** — `scripts/autoregister_project.py` (settings.json:114-124): lee `{cwd}` por stdin y appendea `name→path` a `projects.json`. Sin output a contexto.
- **SessionEnd (session_ingest)** — `scripts/session_ingest_hook.py` (settings.json:162-173): corre `mmorch.session_skills.ingest_workflows(transcript_path)` — 100% local, cero API — y escribe una línea a stderr (`+N workflow obs`, session_ingest_hook.py:28). No inyecta nada al contexto.
- **Stop (context-block)** — `~/.claude/hooks/context-block-watch.js` → `python -m mmorch.context_blocks tick <transcript> <sid>`. `tick()` (context_blocks.py:173-202) estima tokens; sobre el umbral (env `MMORCH_CTX_BLOCK_TOKENS=850000`, settings.json:4) guarda un info-block en SQLite y devuelve UNA línea que el hook manda a **stderr** (watch.js:23). Su par `context-block-reinject.js` (SessionStart matcher `compact`, settings.json:126-135) escribe el último block a **stdout** → se inyecta al contexto fresco post-compact.
- **Nightly evolve** — `scripts/nightly.py` (Windows Task Scheduler `mmorch-nightly`, 02:10, nightly.py:15). Corre: `nightly_evolve()` (findings→PRs, evolve.py:583), autoresearch en worktree, distill en lote (50, mismo watermark `nudge.json`), workflow race + evolve_population, learn_from_repos opcional, arbitration pending. Emite un record JSON por corrida a `logs/nightly.jsonl` (nightly.py:31-37,194).
- **Resumen matutino 09:10** — scheduled task Claude `mmorch-evolve-nightly` (`~/.claude/scheduled-tasks/mmorch-evolve-nightly/SKILL.md`, cron `0 9 * * *`): capa de NOTIFICACIÓN pura — lee la última línea de `logs/nightly.jsonl` y resume en español; explícitamente no ejecuta nada.

### 2. record_outcome / bandit

- **Firma**: `record_outcome(arm: str, reward: float, *, pattern="", predicted_conf=None, source="", context="", path=_FEEDBACK_LOG) -> Outcome` (feedback.py:42-44). Append-only a `logs/feedback.jsonl`; si hay `context`, forward-wire a `intuition.record` (bandit por firma) (feedback.py:56-61).
- **Callers hoy**: code_loop.py:107 (pattern=code_loop, source=execution), rubric_loop.py:271 (rubric_evidence), hillclimb.py:153, project_integrate.py:278 (project_build), evolve.py:354 y evolve.py:508 (`evolve:nightly`, source=human_merge — señal post-merge del humano en reap_merged_prs), feedback_trace.py:79, y el tool MCP `mmorch_record_outcome` (mcp_server.py:561) para labels manuales.
- **`ThompsonBandit`** (feedback.py:85-134): Beta(α,β) por brazo, estado `logs/bandit_state.json`, `update()` con decay 0.995 (half-life ≈139 updates) y write atómico. Instancias separadas: default (cascade/loops), `_SIG_BANDIT` (intuition.py:42), `_WF_BANDIT` (workflow_race.py:82).
- **"Brazo" hoy**: string libre — modelo (`deepseek-chat`), modelo@umbral#contexto vía `contextual_arm()` (feedback.py:65-76), `cascade:stepN`, `evolve:<zone>`, o variante de workflow `name#sig` (workflow_race.py:84).

### 3. Registry de proyectos

- `mmorch/projects.py`: store = `projects.json` plano `{name: abspath}` — sin más campos (projects.py:33-41). `resolve()` valida dir existente (projects.py:57-65); `prune()` GC dry-run default (projects.py:68-79). Autoregistro vía hook SessionStart (autoregister_project.py:44-58), que skipea orchestration/home/Desktop/TEMP.
- **9 proyectos registrados**: Claude, Portfolio financiero, experimentoTrabajo, Proyecto_Adepor, Minecraft, Estudio, Lotus, OS propio, orchestration.
- **Con `.codegraph/`**: 3 — Portfolio financiero, experimentoTrabajo, orchestration.

### 4. Vault

- `vault/`: 19 .md no-babel + 2 `.babel.md`. Estructura: `research/` (13 notas, 2 con babel), `moc/` (3 MOCs autogenerados por `_gen_moc_PROTOTYPE.py`), `memory/` y `roadmaps/` vacíos, más README/lexicon/log/templates.
- **Frontmatter** (ej. research/frugalgpt-cascade-y-model-routing.md:1-8): `title, status (seed|verified|applied|refuted|evergreen|inconclusive), confidence, verifier, tags (incluye tag de proyecto, ej mmorch/estudio), sources, created`. Puerta única validada: `mmorch_vault_write` (mcp_server.py:356-407) — valida title+project tag, regenera MOC, puentea un gist a memoria (`_remember("global", gist, kind="vault_note")`, mcp_server.py:386-389) y dispara babel ingest async.
- **Recall/ranking**: `memory.recall` (memory.py:340) sobre el store SQLite/duckdb semántico (no los .md directos — el gist puenteado es lo que recall encuentra): embedding similarity → pool 3k → `rank_score` (relevancia + recency/decay, retention.py:50) → `mmr_rerank` MMR Jaccard λ=0.7 (retention.py:65, memory.py:372-385). También `recall_keyword` (memory.py:416) y `recall_hybrid` (memory.py:467).

### 5. Roadmap de innovate

- **Roadmap**: `INNOVATION_ROADMAP_2026-06-07.md` (raíz del repo). Último update: commit 86e253f, 2026-06-07 — sin tocar desde entonces. I-1..I-5 todos LANDED según su propia tabla.
- **`mmorch_innovate` NO consume ningún roadmap**: el tool MCP (mcp_server.py:313-328) recibe `context, lenses, ask, rubric` como argumentos del caller y llama `ideate_and_screen` (innovate.py:40-50) — fan_out por lente + adversarial_verify. El roadmap fue un OUTPUT manual de una corrida de junio; no hay lector programático de ese archivo (grep "roadmap" en mmorch/ y mcp_server.py: cero referencias de código).
- **`evolve_open_prs`**: `logs/evolve_open_prs.json` existe y está **vacío** (`{}`). Es el lock por archivo del loop nocturno de PRs (evolve.py:453-515); `reap_merged_prs` lo drena al inicio de cada ronda y registra el outcome post-merge humano (evolve.py:485-515). Vacío = ningún PR nocturno abierto trackeado hoy.

### 6. Hooks globales y presupuesto de latencia

En `~/.claude/settings.json` (líneas 95-174):

| Evento | Hook | Timeout |
|---|---|---|
| UserPromptSubmit | workflow-suggester.js | 5s |
| UserPromptSubmit | wayfinder-suggester.js | 5s |
| SessionStart (todos) | autoregister_project.py | 10s |
| SessionStart (matcher `compact`) | context-block-reinject.js | 15s |
| Stop | context-block-watch.js | 20s |
| PreToolUse (Write/Edit/MultiEdit/NotebookEdit) | never-edit-guard.js | 5s |
| SessionEnd | session_ingest_hook.py | 15s |

Todos fail-open. Presupuestos más holgados: Stop (20s) y SessionEnd (15s); los interactivos (UserPromptSubmit, PreToolUse) toleran solo 5s. El único hook que hoy INYECTA texto al contexto es context-block-reinject.js (stdout en SessionStart:compact); Stop solo escribe a stderr.
