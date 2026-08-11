# Handoff — auditoría EJE=eficiencia — 2026-08-10

Sesión read-only. Entregables: informe (`audit-eficiencia-2026-08-10.md`), entradas de tracker
(`audit-tracker-entries-eficiencia.md`; NO toqué `docs/agents/issue-tracker.md` ni `.scratch/*/issues/`
— el orquestador mergea). Resultado: 0 BLOCKER · 3 IMPORTANTE · 6 NICE-TO-HAVE.

## Qué se cubrió

- **Gates estáticos**: ruff 0 / mypy 0 (sin regresiones; corridos con `.venv/Scripts/python.exe -m ...`
  — ojo: ruff/mypy NO están en el Python global, solo en el venv del repo).
- **Profundo**: mcp_server.py (las 44 tools, surface completa), providers/patterns/ensemble/route/
  budget/metrics/feedback/intuition (hot path de call+route+verify), cache.py (memo), memory.py
  (DuckDB, recall/rerank/remember/consolidate), vault.py (plumbing MOC/write_validated/babel-async),
  context_blocks.py (sqlite), server.py (todos los handlers + frontend polling), config.py, prompts.py
  (prefix-stable), chat_store/workflow_store (conexión módulo-level: OK).
- **Liviano**: ~/.claude/hooks (5 archivos: los suggesters son JS puro sin spawn — OK; context-block-*
  spawnean Python — hallazgo E-7), ~/.claude/skills (markdown estático, sin costo runtime).
- **Greps sistemáticos**: time.sleep/poll (limpio salvo workflow_race delay intencional), timeouts
  (60 s default razonable, documentado H-3), CREATE INDEX (0 en todo el repo — solo relevante en
  context_blocks.db, incluido en E-7), llamadas secuenciales (ensemble = E-2; cascade/tournament/
  code_review secuenciales POR DISEÑO — escalación/eliminación/cadena find→refute; bucketrank ya
  paralelo).
- **Verificación**: todo candidato pasó por `mmorch_adversarial_verify` (2 rondas; la ronda 1 refutó
  mayormente por severidad → tie-break Opus documentado en el informe, apéndice Descartados).

## Qué quedó fuera / débil

- `cascade.py`, `tournament.py`, `code_review.py`, `learn.py`, `predict.py`: solo surface-grep, no
  lectura línea a línea (ninguna señal de grep los marcó).
- `server_engine.py`, `server_core.py`, `server_fleet.py`, `server_pty.py`, `project_build.py`,
  `code_loop.py` y el resto de la capa project/workflow: no leídos en profundidad (no aparecieron en
  greps de eficiencia; son candidatos si otro eje los abre).
- `flywheel/`, `scripts/` (salvo nightly surface), `tests/`, `workflows/*.js`: no auditados.
- No se midió nada en runtime (sin benchmarks); los órdenes de magnitud citados son estimaciones
  razonadas, marcadas como tales.

## Señales para otros ejes

- **Robustez**: (a) `metrics.jsonl`/`feedback.jsonl`/`memo.json` crecen sin rotación ni poda — además
  de costo, es superficie de corrupción/lock en Windows (Memo.put reescribe el archivo entero bajo
  _LOCK de threading pero no inter-proceso: MCP server + server.py + hooks pueden escribir logs
  concurrentemente desde procesos distintos). (b) `budget.check` es fail-closed vía parse de un archivo
  que otro proceso está appendeando — ¿qué pasa con una línea JSON a medio escribir? `read_events`
  hace json.loads sin try por línea (metrics.py:71) → una línea truncada rompe error_rates/budget
  (aunque healthy() traga la excepción fail-open, budget.check NO: una línea corrupta bloquearía
  toda call API con budget seteado). Vale ticket propio de robustez.
- **Seguridad**: `server.py` CORS allow_origins=["*"] + token por query-param (?token=) — ya conocido
  por diseño (tailnet), pero el token en query queda en logs de acceso. `mmorch_vault_write` dispara
  `babel.ingest` en thread daemon con contenido de la nota → revisar qué sale a APIs externas desde
  el vault. No profundicé (fuera de eje).
- **Duplicación estructural** (eje calidad): el patrón "leer JSONL entero + filtrar ventana" está
  copiado en metrics.read_events, feedback.read_outcomes, budget.monthly_spend y schedule.spend_by_period
  — un solo helper con cache lo colapsa (mismo fix que E-1).

## Estado de verificación del entorno

- mmorch MCP server operativo (verify usado desde esta sesión, gemini-3.1-flash-lite, ~$0.002 total).
- metrics.jsonl al momento del audit: 13.581 líneas / ~4 MB; feedback.jsonl 1.878 líneas;
  memo.json ~100 KB (último write jun-08 — el memo casi no se usa: señal de que memoized_verify
  no está en los flujos reales, coherente con E-3).
