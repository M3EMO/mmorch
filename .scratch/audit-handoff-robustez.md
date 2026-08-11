# Handoff — audit EJE=robustez (2026-08-10)

## Qué se cubrió

- **Gates estáticos:** ruff 0, mypy 0 (100 archivos) — sin regresión. Import de los 100
  módulos de `mmorch/` sin fallas (no hay imports rotos).
- **Profundo (leído):** mcp_server.py (tools de vault/recall/outcome/evolve),
  memory.py (DuckDB, digest, distill), vault.py, budget_policy.py, feedback.py,
  cache.py, nudge.py, intuition.py (healthy/decide), evolve.py (Change/evaluate/
  coordinación nocturna de PRs), scripts/nightly.py completo, server_engine.py,
  server_core.py (budget gate), workflow_store.py, chat_store.py, transcript_store.py,
  durable_runs.py, schedule.py, goal.py, metrics.py (write/read), project_build.py
  (caches), sessions.py (parcial).
- **Greps sistemáticos:** except-pass/return (~120 sitios triados), writes no atómicos
  (json.dump/write_text/os.replace), readers jsonl sin tolerancia por línea, paths
  hardcodeados, env-flags muertos (MMORCH_* cruzado contra consumidores — todos vivos),
  pragmas SQLite.
- **Liviano:** los 5 hooks globales (`never-edit-guard` con selftest, context-block-*,
  wayfinder/workflow-suggester — todos fail-open documentado, paths a .venv/python
  existen) y skills (referencias a orchestration válidas). Sin hallazgos ahí.
- **Verificación:** 11 candidatos por `mmorch_adversarial_verify`; 8 sobreviven
  (5 passed + 3 por arbitraje con refutación demostrada como alucinada), 2 descartados,
  2 rondas secas consecutivas sin hallazgos nuevos.

## Qué quedó fuera

- Excluido por consigna: backups, `.scratch/`, contenido del vault, docs.
- No auditado a fondo: server.py/server_fleet.py/server_frontend.py/server_pty.py
  (solo pasadas por grep), flywheel/, plugins/ y plugin_worker.py (protocolo stdio),
  workflows/ JS templates, tests/. pty_session.py y claude_exec.py solo por grep.
- No se corrió ningún flujo (auditoría read-only): las carreras inter-proceso están
  argumentadas por diseño (dos procesos escritores demostrados), no reproducidas.

## Patrón central del eje

Estado en JSON plano con el triple patrón frágil: write no atómico + load que resetea a
default vacío en silencio + read-modify-write sin lock entre MCP server y nightly.py.
Un fix transversal (helper `atomic_write_json` + "corrupt → log fuerte, no default
silencioso" + file-lock o migración a SQLite) resuelve R1, R3, R4, R5 y R7 de una vez.
Las capas SQLite/DuckDB están sanas.

## Señales para otros ejes

- **Ops/observabilidad:** la pata nocturna está EFECTIVAMENTE CAÍDA: task
  `mmorch-nightly` con LastTaskResult=0x800710E0 (refused), StartWhenAvailable=False,
  último registro en logs/nightly.jsonl 2026-08-06 con la máquina activa el 08-10 —
  4 noches sin evolve/distill/race y nadie lo notó. El verificador lo clasificó como
  infra (descartado del informe de robustez), pero ALGUIEN tiene que arreglar el task
  y agregar la alerta por ausencia (el resumidor de 09:00 hoy solo resume lo que hay).
- **Docs/coherencia:** mcp_server.py:370-371,394 promete un "nightly sweep" de babel
  que no existe en scripts/nightly.py ni en babel.py — el except-pass del thread daemon
  se justifica con un componente inexistente. O se implementa la pata (~15 líneas en
  nightly.py) o se corrige el docstring.
- **Eficiencia:** cache.py reescribe memo.json (~100KB) entero por cada put — O(n) por
  escritura; y el re-destilado por watermark perdido (R4) es costo API puro.
- **Meta (calidad del verificador):** gemini-3.1-flash-lite en adversarial_verify
  fabricó código inexistente en 3 de 11 verificaciones (símbolos citados con línea y
  todo). Confirma la lección de MEMORY: el arbitraje necesita ground-truth por lectura,
  y quizás convenga subir el verifier_model default para rubricas de auditoría
  (gemini-3.1-flash no está en el registry — el intento dio KeyError).

## Estado del done-criterio

Checklist 100% recorrido · 2 rondas secas consecutivas (A: durable_runs/goal/
transcript_store/project_build/sessions; B: hooks/skills/env-flags/schema) · 3 archivos
escritos (informe, tracker-entries, este handoff).
