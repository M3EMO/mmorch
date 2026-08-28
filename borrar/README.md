# borrar/ — candidatos a eliminacion (movidos 2026-08-28, el usuario decide)

Cada archivo fue verificado con grep contra mmorch/, scripts/, tests/, tools/, pyproject.toml,
README.md, CLAUDE.md, AGENTS.md, GOAL.md, .beads, .githooks, docs/ antes de moverlo.

- `ablation_adversarial.py` — ablacion suelta de raiz (jun-07); cero referencias en codigo vivo, docs o config.
- `ablation_run.py` — runner de ablacion viejo (jun-07); cero referencias vivas.
- `ablation_selfverify.py` — ablacion suelta (jun-07); cero referencias vivas.
- `build_dataset_repos.py` — script one-shot que poblo .dataset_repos/ (jun-09); nada en mmorch/scripts/tests lo referencia (solo indices derivados .ua/.planning/training, que son data, no codigo).
- `eval_headroom.py` — eval one-off (jun-09); cero referencias vivas.
- `eval_headroom_wsl.py` — variante WSL del anterior (corre con un venv WSL externo); cero referencias vivas.

## Verificado y DEJADO en su lugar (referenciado por algo vivo)
- `ablation_paired.py` — importado por ablation_prompt.py y ablation_symmetric.py (`from ablation_paired import ...`).
- `ablation_prompt.py`, `ablation_symmetric.py` — citados como evidencia en mmorch/checkers.py, mmorch/hillclimb.py, mmorch/patterns.py y README §18.4.
- `AUDIT_2026-06-07.md`, `HANDOFF.md`, `ALGORITHMS-MAP.md`, `WEIGHTS.md` — citados como fuentes en docs/production-readiness/05-known-defects-backlog.md.
- `SELF-EVOLUTION-PLAN.md` — referenciado en mmorch/nodes.py, AGENTS.md y docs/production-readiness/04.
- `HERMES-IDEAS.md` — indexado en AGENTS.md.
- `SETUP-HOST.md` — referenciado por HANDOFF.md (deploy multi-host).
- `brainstorms/2026-06-08-mmorch-ideal-vision.md` — citado como fuente por GOAL.md (zona roja) y SELF-EVOLUTION-PLAN.md; NO existe copia en vault/.
- `.scratch/` (audits ago-2026) — tracked y reciente, insumo del ciclo production-readiness.
- `scripts/register-autopull.ps1`, `scripts/autopull.cmd` — posible registro en Task Scheduler; duda => se queda.

Borrado directo (gitignorado, no vino aca): `__pycache__/` (x6), `.pytest_cache/`, `.pytest-*/` (x18).
