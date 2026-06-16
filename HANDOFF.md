# Handoff — 2026-06-14 · upd 2026-06-16 (+ponytail/lazy, codegraph, sparring→mmorch)

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
1. ✅ **HECHO — Reparar lo funcional**: `mmorch/exec_embedder.py` (`embed_exec`/`embed_hybrid`, cero
   train). **40 specs oracle_diverse: behavioral 0.919 P@1 / 0.948 AUC vs structural 0.430/0.665
   (gap +0.49)**. El structural se desploma al escalar specs; behavioral aguanta. Eval `eval_functional.py`
   (arms code/exec/hybrid, `MMORCH_EVAL_DATA=oracle_diverse.jsonl`). WEIGHTS.md §3c.
2. ✅ **auto-pull task HECHO** (esta PC): Scheduled Task Windows `mmorch-autopull` (cada 15 min →
   `scripts/autopull.cmd` → `mmorch.sync pull-all`, salta dirty). Re-crear: `scripts/register-autopull.ps1`.
   **Pendiente**: always-on en pc-mateo (SETUP-HOST.md; no alcanzo esa PC).
3. ✅ **Fleet-UI HECHO**: selector `destino` en el dashboard (`server.py` _FRONTEND) + `submitJob()`
   rutea local↔`/fleet/run`; botón "usar" por host. Backend `/fleet/run` ya existía.

### Sesión 2026-06-15 (A→E) — además de lo de arriba
- **A** cobertura sondas: drop 8/367→0/221 (float-canon, timeout por-sonda, mutación, callable, sin n=0).
- **B** `embed_hybrid` adapter pluggable construido; shadow_prior contexts = ETIQUETAS no código →
  exec N/A hoy (offline_improvement −0.067 idéntico pa los 3 embed_fn). NO cableado (gated). Pointer en `shadow_prior.py`.
- **C** specs 20→40 (`oracle_dataset.py` +20). Regenerado oracle_dataset.jsonl + oracle_diverse.jsonl
  (backups `.bak` en logs/). Reveló el colapso structural a escala.
- **D** fleet-UI (arriba). **E** PR caveman: branch en fork, gh no instalado → URL compare prefilled
  entregada (no puedo auth desatendido). PR sigue SIN abrir.

### Sesión 2026-06-16 — todo COMMITEADO+PUSHEADO a master (M3EMO/mmorch)
Commits: `ff0da61` exec-embedding (exec_embedder+eval+40 specs) · `65de1fd` fleet-UI · `cd41078`
autopull task · `900f970` docs · `ca618f3` lazy knob · `9ab71a4`+`78653c2` codegraph knob. 309 tests verde.
- **Ponytail evaluado** (repo `DietrichGebert/ponytail`, gemelo de caveman pa CÓDIGO no prosa). Medido:
  corta código −51% LOC SIN romper correctitud (gate filtra). Tokens NO baja en tasks chicos (+2% tax).
  Instalado global como plugin Claude (composa con caveman; único choque statusline=singular).
- **Knob lazy** en `project_loop` (`LAZY_SYSTEM` en `prompts.py`, reglas ponytail vendoreadas, MIT).
  Default ON (`MMORCH_LAZY=0` apaga). System = prefijo estable (cacheable). **Flywheel NO afectado**
  (usa fan_out, no run_project_task) → diversidad de training intacta.
- **Knob codegraph** en `project_loop`: `_codegraph_context()` shell-out al CLI codegraph (mismo motor
  que su MCP, sin protocolo) → contexto del repo como prefijo. **Auto-mantiene índice** (sync si está,
  init+index si no). Gate `MMORCH_CODEGRAPH` (=1 puesto en `.env`). E2E testeado verde (repo temp,
  1 iter, código minimal correcto, .codegraph auto-creado). NO es hook SessionStart a propósito: el
  server corre jobs fuera de sesiones Claude → el knob cubre todos los paths.
- **Statusline merged** `~/.claude/statusline-merged.ps1` (caveman[172]+ponytail[108]+savings suffix),
  wireado en `~/.claude/settings.json`.
- **ACTOR** (framework de estudio del user, proyecto `Desktop/Estudio`): comparte núcleo epistémico con
  mmorch (no-LLM-self-eval, ground-truth, refute-default). Aplicado: `wiki-sparring` ahora rutea el
  refuter a `mmorch_adversarial_verify` (cross-family, cero cupo, $0.0004/run) — commit `555d2a9` en
  M3EMO/Estudio. Own/juicio queda en user+Opus. mmorch YA es ACTOR; único additivo no-hecho = Compress
  discipline en nodos subjetivos (descartado por marginal).
- **Pendiente**: PR caveman sin abrir (igual). chip `test_exec_embedder.py` (exec_embedder sin test). 
  tus otros repos se auto-indexan codegraph al 1er job. server mmorch: reiniciar pa tomar MMORCH_CODEGRAPH.

## Decisions (no re-litigar)
- NO adoptar framework externo (LangGraph/CrewAI) — diluye el determinismo = diferenciador.
- mmorch PRIMARIO en el server (barato), claude -p = escalada. Editar es local al host → GitHub-sync.
- Tailscale (no WireGuard propio). Server idle ≈ 0 carga.
- Weights: torch-train/numpy-infer, manifest+sha, peso=cache regenerable, gate=batir incumbente.
- Ciencia: medir cada lever, no sobre-vender (MoCo rechazado, #1 no-significativo honesto).

## Caveman — sesión 2026-06-15 (proyecto aparte, no mmorch)
Repo `C:\Users\map12\Desktop\Claude\caveman-upstream` (fork `M3EMO/caveman`, upstream `JuliusBrussee/caveman`).
Branch `fix/temp-file-leak` — 1 commit `9ba994f` (Co-author Fable 5), +215/-8, 3 archivos.

**Bug:** `safeWriteFlag` (`src/hooks/caveman-config.js`) escribe flag via temp atómico + `renameSync`.
Windows: `renameSync` sobre destino existente tira `EPERM`/`EBUSY` si otro proceso lo tiene abierto
(statusline leyendo, hook concurrente). El `catch` silencioso se tragaba el error pero nunca borraba
el temp → 1 huérfano por rename fallido. 28 en 2 semanas.

**Fix (3 partes):**
- `safeWriteFlag` → todo en `try/finally`, flag `renamed`; si rename no completó `unlinkSync(temp)`.
- nueva `sweepOrphanTemps(flagDir)`: borra temp solo si >24h **o** PID-muerto + >60s gracia. Regex
  estricta `^\.caveman-active\.(\d+)\.(\d+)$` (nunca matchea flag vivo), `lstatSync` salta symlinks/dirs,
  `pidAlive` trata EPERM como vivo, todo silent-fail. Exportada en `module.exports`.
- wiring: 1 línea en `caveman-activate.js` (SessionStart), corre 1x/sesión.

**Tests:** `tests/test_temp_leak.js` nuevo, 9 casos → **9/9 verde** (verificado).

**Estado:** pusheado a fork `M3EMO/caveman` branch `fix/temp-file-leak`. `PR_BODY.md` listo (untracked).
**PR NO abierto** — `gh` CLI no instalado, sin token/creds usables. Pregunté método (install gh vs URL
prefilled), usuario dismisseó → en espera.

**Next:** abrir PR upstream `JuliusBrussee/caveman:main` ← `M3EMO:fix/temp-file-leak` con `PR_BODY.md`.
Opciones: `winget install GitHub.cli` + `gh auth login`, o URL compare prefilled pa click manual.

## Read first
`WEIGHTS.md` (resultados flywheel + cómo armar pesos), `SETUP-HOST.md` (deploy multi-host),
`AGENTS.md`/`GOAL.md` (contrato), `SELF-EVOLUTION-PLAN.md` §BACKLOG (seeds: exec-embedding,
GNN-AST, DSPy, fleet-UI). Memoria: [[mmorch-platform]], [[flywheel-simclr-result]], [[mmorch-harness]].
WSL torch `~/flywheel/bin/python`; paths /mnt/c → `MSYS2_ARG_CONV_EXCL='*'` (y NO usar $var en for-loops wsl).
