# Handoff — 2026-06-14

## Goal
mmorch = orquestador determinista multi-modelo (ahorra cupo Claude). Esta etapa: plataforma de
agente (server live + fleet + project-aware + sync) + flywheel del code_embedder. Repo:
github.com/M3EMO/mmorch (push activo, ~v1.2). Todo goal-gated, cross-family, red-zone nunca autónomo.

## State — 309 tests verde, ~52 módulos, ~95 commits, pusheado
**Plataforma (esta sesión):**
- **Live server** `server.py` (Starlette+uvicorn, cero dep nueva): SSE progreso por subagente +
  control remoto + token. Bus `events.py`. Dashboard: **Kanban** (jobs por status) + panel **fleet**.
  CORRIENDO en tailnet `http://100.113.221.3:8787` (token `bfP0brI-if387ExSyUD6-uZm`) — background
  de ESTA sesión (muere al cerrar; pa always-on ver SETUP-HOST.md).
- **Fleet** `fleet.py` (hosts.json + estado agregado + forward). **project-aware**: `projects.py`,
  `project_loop.py` (PRIMARIO mmorch: DeepSeek genera+tests verifican+aplica; claude -p escalada
  via `claude_exec.py`). **sync.py** (GitHub bus: edit→push branch mmorch/auto, auto-pull seguro).
- **auto-register** hook SessionStart (~/.claude/settings.json). **packaging** `pyproject` v1.2
  → `pip install -e .[host]` + scripts mmorch-server/mmorch-sync. **weights** manifest+sha (`weights.py`).
- **enrich.py** (intent completion, guard cross-family).

**Flywheel (ablación hoy, en WEIGHTS.md):**
- retrain full-config: 0.88→**0.899** (dim 384) ✅ promovido. **fp16** ½ tamaño lossless ✅.
- **#2 MoCo RECHAZADO** (0.884<0.899, dataset chico). **#1 functional positives**: +0.024 P@1
  (5 seeds, 4/5 pos, 1 neg) — DIRECCIONAL, NO significativo → NO promovido. Cuello = spec-count.
- **Hallazgo clave**: el encoder es ESTRUCTURAL, no funcional (colapsa 0.99→0.45 en data diversa).

## Next
1. **Reparar lo funcional** (la pregunta abierta): el fix REAL = **embedding por EJECUCIÓN**
   (huella de comportamiento: correr en N inputs-sonda → vector outputs = funcional exacto, cero
   train). SEED nuevo en SELF-EVOLUTION-PLAN. Alternativa incremental: escalar #1 a 40-100 specs.
2. Always-on en pc-mateo (SETUP-HOST.md; yo no alcanzo esa PC) + auto-pull task en esta PC.
3. Fleet-control UI (routing a host elegido) — backend listo, falta UI.

## Decisions (no re-litigar)
- NO adoptar framework externo (LangGraph/CrewAI) — diluye el determinismo = diferenciador.
- mmorch PRIMARIO en el server (barato), claude -p = escalada. Editar es local al host → GitHub-sync.
- Tailscale (no WireGuard propio). Server idle ≈ 0 carga.
- Weights: torch-train/numpy-infer, manifest+sha, peso=cache regenerable, gate=batir incumbente.
- Ciencia: medir cada lever, no sobre-vender (MoCo rechazado, #1 no-significativo honesto).

## Read first
`WEIGHTS.md` (resultados flywheel + cómo armar pesos), `SETUP-HOST.md` (deploy multi-host),
`AGENTS.md`/`GOAL.md` (contrato), `SELF-EVOLUTION-PLAN.md` §BACKLOG (seeds: exec-embedding,
GNN-AST, DSPy, fleet-UI). Memoria: [[mmorch-platform]], [[flywheel-simclr-result]], [[mmorch-harness]].
WSL torch `~/flywheel/bin/python`; paths /mnt/c → `MSYS2_ARG_CONV_EXCL='*'` (y NO usar $var en for-loops wsl).
