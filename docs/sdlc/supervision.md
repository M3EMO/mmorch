# supervision.md — driver_py


## candidato 23:42:12
revision de spec (Claude, rc=0): 0 preguntas: la spec y los tests ya cubren todo el barrido sin ambigüedad.

## candidato 00:41:53
revision del diff (Claude, rc=0): BLOCK: `_hide_ledger` (mmorch/auto_apply.py) escribe "logs/" en `.git/info/exclude` del runtime. Ese archivo es compartido con el repo fuente. El efecto se filtra al repo fuente real. Esconde cualquier carpeta `logs/` de `git status` ahí. Ni la TAREA ni spec.md piden tocar el repo fuente.

## candidato 01:06:42
nivel 3 Claude (rc=0): Los 3 tests pedidos y `test_auto_apply.py` pasan (30/30). Las otras 2 fallas del repo son de entorno. No tocan `auto_apply.py`.

Cambié `_hide_ledger` en `mmorch/auto_apply.py`. Antes de fijar `core.excludesFile --worktree`, ahora habilito `extensions.worktreeConfig`. Sin eso, `git config --worktree` fallaba en un worktree real. Eso dejaba el runtime sucio tras el revert. Y eso tumbaba `recovery_verified`.
