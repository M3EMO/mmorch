# Handoff — 2026-09-02 — auto-apply loop

## Goal
Cerrar el circuito de mmorch para promover cambios no-rojos en un runtime aislado,
observarlos y revertir automáticamente cualquier regresión.

## State
- Done: contrato y rollout en `.scratch/auto-apply-loop/spec.md`; `GOAL.md` intacto.
- Done: estado durable `candidate → merged → observing → accepted` y
  `observing → reverting → reverted → halted` en `mmorch/promotion.py`.
- Done: runtime persistente fuera del checkout humano en `mmorch/runtime_checkout.py`.
- Done: preflight fail-closed y merge/revert/observe en `mmorch/auto_apply.py`.
- Done: política 26 h / 2 muestras en `mmorch/observation.py`.
- Done: adapter nocturno con rollout `off|shadow|green|yellow_bounded|non_red` en
  `mmorch/auto_apply_nightly.py`; `nightly.py` lo llama, default `off`.
- Done: guardrails nuevos agregados a `_RED_PATHS` en `mmorch/evolve.py`.
- Done: tests nuevos: `test_promotion.py`, `test_runtime_checkout.py`,
  `test_auto_apply.py`, `test_observation.py`, `test_auto_apply_nightly.py`.
- Done: E2E Git real prueba merge → regresión → revert → halt y checkout humano intacto.
- Done: focales observados: 65, 48, 55 y 59 tests verdes; lints IDE sin errores.
- Done: suite canónica `1186 passed, 2 warnings in 542.98s`, exit 0.
- Done: automerge legacy de `hardening.py`/`auto_repair.py` diferido al runtime.
- Done: reconciliación mid-Git probada antes/después de merge y después de revert;
  focales posteriores: 53 passed.
- Done: gates estáticos verdes: ruff, mypy (137 módulos), paths y docgen.
- Done: `scripts/nightly_task.cmd` probado con `--help`; fija cwd y captura output.
- Blocked: smoke 12/13; `digest`/`nightly` vencidos porque el task no completa.
- In progress: Mateo actualizó el task con su credencial; action apunta a
  `scripts/nightly_task.cmd`. Corrida real iniciada 17:01:37, cmd PID 9616 y Python
  hijo PID 3432 confirmados; output en `logs/nightly_task.out`.
- Pending: suite canónica posterior a los últimos cambios.
- Not activated: no se creó runtime real y no se habilitó
  `MMORCH_AUTO_APPLY_ROLLOUT`.

## Next
1. Esperar fin del task: `schtasks /Query /TN "mmorch-nightly" /V /FO LIST`;
   esperado `Estado: Listo` y `Último resultado: 0`.
2. Correr smoke; esperado 13/13.
3. Repetir suite completa y correr varios ciclos shadow controlados.
4. Presentar evidencia y pedir GO/NO-GO para runtime/amarillo.

## Decisions
- Destino elegido por Mateo: todo no-rojo, sólo mmorch.
- Rollout obligatorio: shadow → green → yellow_bounded → non_red; no salto directo.
- Checkout humano nunca recibe merge/reset/revert automático.
- Todo gate ambiguo/caído falla cerrado para auto-apply; puede seguir abriendo PR humano.
- Budget configurado y comando `MMORCH_AUTO_APPLY_RESTART` son obligatorios para aplicar.
- Una regresión revierte, verifica recuperación y deja `loop_paused` aunque el revert funcione.

## Known gaps / blockers
- Shadow real ejecuta suite+gates+smoke+canary y requiere runtime/API/budget; aún no se corrió.
- Task Scheduler reparado con wrapper que fija cwd y captura stdout/stderr; falta
  que la primera corrida termine con exit 0.
- Una corrida manual fue terminada a ~30 min durante autoresearch; no prueba hang.
- El warning `pytest` de cleanup `pytest-current`/WinError 5 aparece después de tests verdes;
  no confundirlo con fallo de assertions, pero confirmar el exit code.

## Read first
- `.scratch/auto-apply-loop/spec.md`
- `mmorch/auto_apply.py`
- `mmorch/auto_apply_nightly.py`
- `mmorch/promotion.py`
- `tests/test_auto_apply.py`
- este archivo
