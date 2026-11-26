@AGENTS.md

# mmorch — Orquestacion Multi-Modelo

Root reemplaza a mmorch (decision del usuario 2026-10-31). Una idea nueva de banco,
oraculo o ruteo va a Root, no a mmorch. Este repo solo recibe mantenimiento.

## Invariantes medidos (vinieron del CLAUDE.md global el 2026-11-26)
- **Checkeable → oraculo, nunca un LLM.** `checkers.py`, no un juez: el LLM-juez refuta en falso ~74% en checkeable dificil.
- **Verificador con razonamiento prendido.** Medido 2026-09-07 (n=350): sin `thinking` rechaza ~40% del trabajo
  correcto (acc. balanceada 0.77); con `thinking`, 0.997. Para verificar: `DEFAULT_VERIFIER` (`mmorch/config.py`), que razona.
- Verificar barato: `/verify-cross`. Numeros y como re-correrlos: seccion "Measured" del README.

Mapa de fuentes: `docs/SOURCES.md`. Vocabulario: `CONTEXT.md`. Invariantes: `GOAL.md`
(no se reescriben aca). Catálogo vivo: `docs/generated/catalog.md`.

Recurso escaso = **cupo** del plan Claude (no dolares). Generacion masiva y
verificacion se delegan a APIs externas baratas para **liberar cupo**.
Libreria: `~/.claude/orchestration/` (paquete `mmorch`). Tambien MCP `mmorch`.
Lista de tools: `mcp_server.py` (vista: catalogo generado). Version: `pyproject.toml`.
Reload Claude Code para cargar tools nuevos.

## Decision dura: cupo (Workflow nativo) vs API barata (mmorch)
- **Flujo recurrente/entendido** (bulk gen, verificar, rutear repetido) → `mmorch`
  (API externa, **cero cupo**). Es la palanca de ahorro.
- **Flujo novel/one-off de alto valor** (perspectivas independientes, goal drift) →
  Workflow nativo de Claude Code (gasta cupo). Sujeto al opt-in gate.
- Nunca delegar a `mmorch` lo que necesita contexto/juicio del orquestador (Fase 0/1,
  sintesis critica, tie-break) — eso es Opus.

## Reglas de turno (apuntes; el contrato es GOAL.md)
- **Verificador.** Familia libre desde 2026-10-07 (cross-family no mejoro en checkeable;
  sesgo de familia en subjetivo < 7.5 pts). Default `DEFAULT_VERIFIER` (`mmorch/config.py`): razona, refuta por
  default y es el mas barato medido. Anti-sicofancia: el acuerdo no es confirmacion.
- **Anti-reward-hacking.** En `hillclimb`, `score` = checker o comando — nunca LLM-judge.
- **Observabilidad.** Cada nodo loggea a `logs/metrics.jsonl`. Sin metricas no hay
  break-even.
- **Modelos.** Roles y precios: `mmorch/config.py`. Desde 2026-10-26, Haiku 5.5
  reemplaza a zhipu (sin saldo) por API, en dolares y sin cupo.

## SDLC = builder, mmorch = banco
- El pipeline de 6 etapas vive en el paquete `sdlc/` (decision del usuario 2026-10-22). Construye; no mide.
- `sdlc` importa de `mmorch`; mmorch solo lo importa para correrlo (server, nocturno) o medirlo (`mmorch.sdlc_bench`).
- Un cambio al builder se juzga con tests, gates y corridas reales, sin A/B obligatorio del bench.

## Donde leer el resto
- Hilo con Cursor: `logs/canal.jsonl` (`mmorch.canal`). Nadie despierta al otro.
- Delegar implementacion a Cursor: `python -m mmorch.cursor_worker correr <repo> <pedido.md>` en segundo plano;
  despues `corregir` (una vez), `aplicar` o `descartar`. Contrato y limites: docstring de `mmorch/cursor_worker.py`.
- Capacidades (cuando elegir un patron): `docs/capabilities.md`
- Feedback y memoria (como estan hechos): `mmorch/feedback.py`, `mmorch/memory.py`
- Tests: `tests/` es el gate para promover codigo nuevo
- Auto-evolucion: gated; nunca auto-modifica vivo sin tests + gate humano.
  Motor: `mmorch/evolve.py`. Research: `vault/`
- Prosa STE: `python tools/ste-lint.py docs/*.md --fail-over 5` (`--lang es` ok)

Mediciones de verificadores: seccion "Measured" del README. No escalar sin metricas verdes.


