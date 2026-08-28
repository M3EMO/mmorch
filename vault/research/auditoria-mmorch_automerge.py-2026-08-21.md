---
title: auditoria mmorch/automerge.py 2026-08-21
status: seed
tags: [mmorch, self-audit]
created: 2026-08-21
---

El módulo implementa automerge con semáforo de zonas. Se detectan 4 findings: un bug en la detección de contenido rojo en archivos nuevos, violación del ADR 0001 al releer el diff del disco, acoplamiento oculto con evolve._RED_PATHS, y falta de validación del gate de tests.

## Findings (sobrevivieron refutacion 4/4 — 1 estructurales, 1 bugs, 1 de principios)

- **try_automerge relee el diff del disco violando ADR 0001** [media/estructural]: try_automerge llama a classify_branch que ejecuta git diff y git show (3+ subprocess calls). El caller ya tiene el estado del repo en memoria. El módulo debería recibir el diff como parámetro en lugar de releer del disco, violando el principio 'estado en memoria, no releer disco' del ADR 0001.
- **red_content_hits marca archivos nuevos con fixtures de tests como rojos** [alta/bug]: En classify_branch, para archivos con status 'A' (nuevos), se pasa baseline='' a red_content_hits. Si un test nuevo contiene strings como 'password' o 'secret' en fixtures (común en tests), se marcará como rojo incorrectamente. El semáforo debería ignorar contenido rojo en archivos nuevos que son tests, ya que no pueden afectar producción.
- **Acoplamiento oculto con evolve._RED_PATHS y red_content_hits** [media/principio]: El módulo importa _RED_PATHS (privado) y red_content_hits de mmorch.evolve dentro de la función. Si evolve cambia la firma de red_content_hits o renombra _RED_PATHS, este módulo revienta silenciosamente. Debería haber una interfaz pública en evolve que encapsule la lógica del semáforo.
- **try_automerge no valida que el caller haya pasado el gate de tests** [baja/otro]: El docstring dice 'el caller ya paso el gate de ejecución' pero try_automerge no verifica esto. Si un caller futuro llama sin pasar el gate, el merge verde se ejecutará sin validación. El módulo debería recibir un parámetro tests_passed: bool y abortar si es False, o al menos loggear una advertencia en el ledger.
