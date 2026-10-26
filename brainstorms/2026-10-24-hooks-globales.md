# Hooks globales de Claude Code: Brainstorm / Discovery Notes
Date: 2026-10-24 · Goal: decidir qué cambia en `~/.claude/settings.json` y `~/.claude/hooks/` (bd `orchestration-wz1`).

## Summary / key decisions
- D1: sacar `workflow-suggester` y `wayfinder-suggester` de UserPromptSubmit (Q2).
- D2: sacar `context-block-watch` y `context-block-reinject` (Q3).
- D3: sacar `ets-remind`; ETS queda en `~/.claude/CLAUDE.md` + `ets-guard` (Q4).
- D4: sacar el hook `bd prime --hook-json` (Q5); en orchestration y mf-moe; `sdlc bd` va a un issue aparte (Q6).
- D5: sacar `proposal_hook` en la laptop y en el escritorio (Q7).
- D6: el hook de impacto (pre y post) solo dispara en archivos `.py` (Q8).
- D7: sacar `session_ingest_hook` en las dos máquinas (Q9).
- D8: limpieza (a), (b) y (c) de Q11; los permisos de Windows quedan.
- D9: ponytail queda en full (Q12, Q13).

## Inventario medido (2026-10-24, laptop Fedora)

Tiempo = una corrida con stdin de prueba, proceso frío. Los hooks del mismo evento corren en paralelo.

| Evento | Matcher | Script | Qué hace | Tiempo |
|---|---|---|---|---|
| UserPromptSubmit | — | `hooks/workflow-suggester.js` | Regex sobre el prompt; sugiere `dynamic-workflows` | 93 ms |
| UserPromptSubmit | — | `hooks/wayfinder-suggester.js` | Regex sobre el prompt; sugiere wayfinder | 82 ms |
| UserPromptSubmit | — | `hooks/ets-remind.js` | Inyecta las reglas ETS en cada prompt | 89 ms |
| UserPromptSubmit | — | plugin ponytail `ponytail-mode-tracker.js` | Sigue el nivel de ponytail | no medido |
| SessionStart | — (todos) | `orchestration/scripts/autoregister_project.py` | Registra el cwd en `projects.json` | 23 ms |
| SessionStart | `compact` | `hooks/context-block-reinject.js` | Reinyecta el info-block guardado tras compactar | 1339 ms |
| SessionStart | `startup` | `orchestration/scripts/proposal_hook.py` | Inyecta la mejor propuesta del loop de ideas | 876 ms |
| SessionStart | startup/resume/clear/compact | plugin ponytail `ponytail-activate.js` | Inyecta el prompt de ponytail + mapa del código | no medido |
| SessionStart | — | `bd prime --hook-json` (solo `.claude/settings.json` de orchestration) | Inyecta el contexto de bd | 161 ms |
| Stop | — | `hooks/context-block-watch.js` | Estima tokens del transcript; guarda info-block al umbral | 1802 ms |
| Stop | — | `hooks/ets-guard.js` | Mide R1-R3 de ETS; reporta, no bloquea | 191 ms |
| PreToolUse | Write/Edit/MultiEdit/NotebookEdit | `hooks/never-edit-guard.js` | Niega edición de rutas en `never-edit.txt` | 209 ms |
| PreToolUse | Write/Edit/MultiEdit/NotebookEdit | `orchestration/scripts/impacto_hook.py hook` | Informe de literales compartidos antes de editar | 2067 ms |
| PostToolUse | Write/Edit/MultiEdit | `orchestration/scripts/impacto_hook.py post` | Parte post del informe de impacto | no medido |
| SessionEnd | — | `orchestration/scripts/session_ingest_hook.py` | Mina playbooks de la sesión (local) | no medido (escribe) |
| SubagentStart | — | plugin ponytail `ponytail-subagent.js` | Inyecta ponytail en subagentes | no medido |

### Observaciones del inventario
- Los scripts viven en dos lugares: 6 `.js` en `~/.claude/hooks/` y 4 `.py` en `orchestration/scripts/`.
- Cada Stop paga ~1.8 s de `context-block-watch` (arranca Python).
- Cada Write/Edit paga ~2.1 s de `impacto_hook` antes y otra corrida después.
- Los dos hooks de impacto no tienen `statusMessage`; el resto sí.
- `ets-remind` repite reglas que ya están en `~/.claude/CLAUDE.md`.
- El encabezado de `ets-guard.js` dice "decision: block", pero el código no bloquea desde 2026-09-10.
- `permissions.allow` todavía tiene entradas de Windows (`.venv/Scripts/python.exe`, `dir`, `type`, `move`, `del`).

## Q&A log
### Q1 — Qué molesta
- Asked: ¿qué te molesta hoy de los hooks globales?
- Captured: el usuario marcó lentitud, ruido en el contexto y hooks que no sirven. Agregó: "Conflictividad entre unos y otros. Y utilidad, quiero ver si son utiles".
- No marcó "dos carpetas de scripts": la ubicación no es prioridad.
- Siguiente: medir conflictos y utilidad de cada hook con datos, no con opinión.

### Q2 — Sugeridores de workflow y wayfinder
- Asked: dispararon 138 veces y nunca llevaron a su skill; ¿qué hago?
- Captured: el usuario eligió "Sacar los dos". Las reglas del gate y de wayfinder quedan en `~/.claude/CLAUDE.md`.
- Nota: el usuario rechazó una tanda de 4 preguntas juntas; el grilling sigue con una pregunta por vez.
### Q3 — Par context-block watch/reinject
- Asked: el watch cuesta 0,8 s por Stop y su bloque se reinyectó 7 veces en 3005 corridas; ¿qué hago?
- Captured: el usuario eligió "Sacar los dos". Salen `context-block-watch.js` (Stop) y `context-block-reinject.js` (SessionStart:compact).
### Q4 — Tres vías de ETS
- Asked: ETS llega por CLAUDE.md, `ets-remind` y `ets-guard`; ¿qué dejo?
- Captured: el usuario eligió "CLAUDE.md + ets-guard". Sale `ets-remind.js` de UserPromptSubmit. `ets-guard` ya pide no repetir el mensaje anterior.
- Consecuencias de D1 y D2 (a confirmar al final): `~/.claude/CLAUDE.md:48` nombra los sugeridores; `mmorch/context_blocks.py` y `env.MMORCH_CTX_BLOCK_TOKENS` quedan huérfanos; `tests/test_capas.py:26` cita los hooks; `docs/cursor-setup.md:91` nombra `workflow-suggester`.
### Q5 — Conflicto de `bd prime`
- Asked: `bd prime` contradice las reglas propias de bd en cada sesión de orchestration; ¿cómo lo resuelvo?
- Captured: el usuario eligió "Sacar el hook de bd prime".
- Hallazgo posterior: el hook vive en `.claude/settings.json` de cada repo (orchestration, mf-moe), no en el global. `python -m sdlc bd <repo>` (`sdlc/agentes.py:190`) corre `bd setup claude`, que lo vuelve a agregar, y `bd setup claude --check` lo exige. Pendiente: alcance de D4 (Q6).
### Q6 — Alcance de D4
- Asked: sacar `bd prime` choca con `python -m sdlc bd`, que lo reinstala y lo exige; ¿qué alcance tiene D4?
- Captured: el usuario eligió "Issue aparte para sdlc bd". Hoy salen los hooks de orchestration y mf-moe. Un issue bd nuevo cambia `sdlc bd` para que quite el hook y no corra `--check`.
### Q7 — `proposal_hook`
- Asked: tarda 0,9 s por inicio y en la laptop nunca muestra nada (falta `logs/adjudications.json`); ¿qué hago?
- Captured: el usuario eligió "Sacarlo en las dos máquinas".
- Flag: el `settings.json` del escritorio Windows no es accesible desde esta sesión.
### Q8 — Hook de impacto
- Asked: avisa en 25 de 127 ediciones (20 %) y paga hasta 4,4 s antes (p90) y 2,6 s después (mediana); ¿qué hago?
- Captured: el usuario eligió "Filtrar por extensión". Los dos hooks quedan, pero solo disparan en Write/Edit de archivos `.py`.
- Verificado en https://code.claude.com/docs/en/hooks: el campo `if` acepta una sola regla de permiso, por ejemplo `Edit(**/*.py)`. Un handler por herramienta: `Edit(**/*.py)` y `Write(**/*.py)`. Falta probar `Write(...)` en una sesión nueva.
### Q9 — `session_ingest_hook`
- Asked: mina cada sesión al cerrar, pero en la laptop nadie lee `logs/workflow_obs.jsonl` (sin loop nocturno, `logs/` ignorado por git, `mmorch_session_playbooks` con 0 llamadas); ¿qué hago?
- Captured: el usuario eligió "Sacarlo en las dos máquinas".
### Q10 — Plugin ponytail
- Asked: ponytail inyecta ~4,6 k caracteres en cada inicio y compactación; ¿qué hago?
- Captured: el usuario respondió "Medir su efecto off, lite y full. Con esos datos decido. Testealo pidiéndole a agentes la misma tarea de crear un archivo con x funciones".
- Pendiente: experimento off/lite/full (ver sección Experimento ponytail).

## Experimento ponytail (Q10)
- Tarea: crear `funcs.py` con 8 funciones de spec cerrada. Oráculo: 30 asserts ocultos, validados con una implementación de referencia (30/30).
- Brazos: `off` (sin plugin), `lite` y `full` (`--plugin-dir` + `PONYTAIL_DEFAULT_MODE`). 5 corridas por brazo, `claude -p --model sonnet`.
- Aislamiento: `--setting-sources project --strict-mcp-config`, sin hooks del usuario ni MCP. El CLAUDE.md global sí carga, igual en los tres brazos.
- Métricas: tests pasados, líneas de `funcs.py`, tokens de salida, costo, segundos, turnos, archivos extra.
- Material: `scratchpad/ponytail-exp/` de la sesión (`task.md`, `test_hidden.py`, `run.sh`, `score.py`).
- Piloto `full-0`: 30/30, 69 líneas, 2286 tokens de salida, US$ 0,074, 18 s.
- Resultado (n=5 por brazo, sonnet-5-5):

| Brazo | Tests | Líneas | Tokens salida | Costo | Segundos |
|---|---|---|---|---|---|
| off | 30/30 | 59,6 | 1796 | US$ 0,062 | 14,4 |
| lite | 30/30 | 68,0 | 2007 | US$ 0,070 | 16,2 |
| full | 30/30 | 67,0 | 1997 | US$ 0,070 | 16,0 |

- Causa de las líneas extra: lite y full agregan un bloque `if __name__ == "__main__"` con 8-10 asserts (regla "self-check" de ponytail). off no agrega ninguno. El código de las funciones mide lo mismo.
- lite y full dan igual en esta tarea. Ponytail cuesta ~13 % más y no cambia la corrección (techo 30/30).
- Límite: una sola tarea, de spec cerrada y fácil. No mide el efecto anti-sobreingeniería en tareas abiertas.

### Q12 — Decisión de ponytail tras la tanda 1
- Asked: con la tanda 1, ¿qué hago con ponytail?
- Captured: el usuario respondió "1 + complejo": medir otra tanda con una tarea abierta y más compleja antes de decidir.
- Tanda 2 (tarea abierta `ledger.py`, "cómodo y robusto", 12 asserts ocultos, n=5 por brazo):

| Brazo | Tests | Líneas ledger | Líneas totales | Tokens salida | Costo | Segundos |
|---|---|---|---|---|---|---|
| off | 12/12 | 94,4 | 94,4 | 2960 | US$ 0,075 | 22,4 |
| lite | 12/12 | 67,6 | 84,2 | 2696 | US$ 0,081 | 21,4 |
| full | 12/12 | 60,8 | 60,8 | 2163 | US$ 0,070 | 17,6 |

- full escribe 36 % menos código, 27 % menos tokens y tarda 21 % menos que off, con la misma corrección.
- off usa `tempfile` en los 5 casos: guarda de forma atómica. Esa robustez no la miden los asserts.
- lite es el más variable: 2 de 5 corridas escriben un `test_ledger.py` aparte y suben a 5 turnos.
- Material: `scratchpad/ponytail-exp2/`.

### Q13 — Decisión de ponytail
- Asked: con las dos tandas, ¿qué hago con ponytail?
- Captured: el usuario eligió "Dejarlo en full".

### Q11 — Limpieza de restos
- Asked: ¿qué restos limpio en este issue?
- Captured: el usuario aprobó (a) borrar `~/.claude/CLAUDE.md:48` y `env.MMORCH_CTX_BLOCK_TOKENS`; (b) corregir el encabezado de `ets-guard.js` y agregar `statusMessage` a los hooks de impacto; (c) borrar `mmorch/context_blocks.py` y su mención en `tests/test_capas.py`, con tests.
- No aprobó sacar los permisos de Windows de `permissions.allow`.

### Q14 — Confirmación del plan
- Asked: ¿aplico el plan de 13 pasos (settings global, 5 `.js`, CLAUDE.md global, `ets-guard`, `bd prime` en orchestration y mf-moe, `context_blocks.py`, issue de `sdlc bd`, sin commit)?
- Captured: el usuario eligió "Aplicar todo". Sin commit.

## Aplicado y verificado (2026-10-26)
- `~/.claude/settings.json`: D1-D3, D5-D7 y la variable de D8. Respaldo previo en el scratchpad de la sesión (`settings.bak.json`).
- Borrados los 5 `.js` sin uso; `~/.claude/hooks/` queda con `ets-guard.js` y `never-edit-guard.js`.
- `~/.claude/CLAUDE.md`: salen las líneas de los sugeridores. `ets-guard.js`: encabezado corregido.
- `bd prime` fuera de `.claude/settings.json` de orchestration y mf-moe (los dos quedan `{}`). Issue `orchestration-1hd` para `sdlc bd`.
- `mmorch/context_blocks.py` borrado; `tests/test_capas.py`, README y catálogo actualizados. Suite: 1421 passed; `docgen --check` OK.
- Sesión nueva headless en un repo de prueba: impacto corrió en `Edit a.py` y `Write c.py`, no en `Write notas.md`; el post también corrió.
- Sesión nueva headless en orchestration: inicio sin `bd prime`, con ponytail full, sin nudges de prompt.
- Sin commit, por decisión del usuario (Q14).
- 2026-10-26: el usuario pidió borrar `scripts/proposal_hook.py` y `scripts/session_ingest_hook.py`; ningún test ni módulo los llamaba.
- Cascada (2026-10-26): el borrado dejó sin llamador a `pick_card` y a `mmorch/decision_mining.py`; el usuario aclaró que el escritorio Windows ya no existe y se borraron con sus tests. `sessions.redact` quedó sin llamador; el usuario eligió cablearla en `scripts/export_training_data.py` (`_write` redacta cada string), con `tests/test_export_redact.py`. Suite: 1409 passed.

## Utilidad y conflictos medidos (157 transcripciones, `~/.claude/projects/*/*.jsonl`)

Script: `scratchpad/scan.py` de la sesión (no versionado). Duración = `durationMs` del harness.

| Hook | Veces | Efecto medido |
|---|---|---|
| `context-block-watch` (Stop) | 3005 corridas, mediana 0,8 s, p90 2,5 s | Guardó 697 info-blocks; `reinject` los usó 7 veces |
| `ets-guard` (Stop) | 2667 corridas, mediana 0,12 s | Marcó violaciones en 521 respuestas (20 %); inyectó 151 k caracteres |
| `ets-remind` (prompt) | 1774 inyecciones | 662 k caracteres; repite la sección ETS de `~/.claude/CLAUDE.md` |
| `workflow-suggester` (prompt) | 119 disparos | `dynamic-workflows` se invocó 0 veces en toda la historia; tras la señal hubo 3 `Workflow` y 1 `workflow-authoring` |
| `wayfinder-suggester` (prompt) | 19 disparos | El skill `wayfinder` se invocó 0 veces |
| `impacto_hook` (Edit) | ~19 informes con contenido | Mediana 0,24 s, p90 4,4 s; el post mediana 2,6 s |
| `bd prime` (inicio, solo orchestration) | 35 corridas, mediana 0,7 s | ~5,8 k caracteres por sesión |
| ponytail activate (inicio y compact) | 176 corridas, mediana 0,3 s | Inyecta prompt + mapa del código |

### Conflictos encontrados
- C1: `bd prime` prohíbe TodoWrite, manda `bd remember` y prohíbe MEMORY.md. `AGENTS.md` ("Reglas propias de bd") y `~/.claude/CLAUDE.md` dicen lo contrario. El modelo recibe las dos órdenes en cada sesión.
- C2: tras compactar inyectan a la vez ponytail completo, `bd prime` completo y el info-block. Son tres bloques largos en el mismo momento.
- C3: `ets-remind` y `~/.claude/CLAUDE.md` dicen lo mismo; `ets-guard` agrega un tercer aviso por turno con 20 % de respuestas marcadas.
- C4: `workflow-suggester` dispara en 119 prompts y casi nunca lleva a un workflow. La regla del gate ya vive en `~/.claude/CLAUDE.md`.

## Open flags (pending input)
