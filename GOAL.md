# mmorch — GOAL (north-star contract)

> Ancla anti-goal-drift. Modelado sobre el `/goal` nativo de Claude Code (condición +
> Stop-hook que bloquea "done"): acá el GOAL bloquea la AUTO-APLICACIÓN. El loop de
> auto-evolución no cierra un ciclo hasta que el cambio pasa `goal_aligned()` contra
> este archivo. **Editar este archivo es ZONA ROJA — requiere gate humano explícito.**
> (fuente: brainstorms/2026-06-08-mmorch-ideal-vision.md)

## North star
Un orquestador multi-modelo **auto-evolutivo, inteligente, seguro y barato**: idea →
verifica → prototipa → aplica mejoras a sí mismo, cerrando el ciclo de auto-evolución
con auto-aplicación PROGRESIVA y reversible, liberando cupo de Claude vía modelos
externos baratos. Camino: A (router cerebral) → B (cerebro meta) → C (operador), A primero.

## Invariantes (SIEMPRE se sostienen; violarlos = cambio rechazado)
- **Zona roja innegociable** (nunca solo, gate humano): mover dinero/trades/suscripciones/claves;
  borrar datos fuera del sandbox o vaciar memoria; tocar SO/red/hardware fuera del repo;
  modificar las propias políticas de seguridad o este GOAL; comunicaciones externas en
  nombre del usuario; eliminar la capacidad de rollback.
- **OneFlow / cross-family**: generador y verificador de familias distintas en tareas
  subjetivas; same-family solo en checkeable. El acuerdo NO es confirmación (anti-sicofancia).
- **Reversibilidad first-class**: nada se auto-aplica (verde/amarillo) sin `rollback()`
  implementado y PROBADO. Si rollback falla, el cambio nunca debió auto-aplicarse.
- **Gate antes de aplicar**: `fitness()` (tests verdes + checkers + ensemble + rollback +
  no-degradación de costo + `goal_aligned`) pasa o el cambio se aborta.
- **Costo acotado**: respeta el BudgetKeeper; nunca gasta por encima del límite sin override.
- **Observabilidad**: toda auto-acción → episodio `kind="auto_action"` auditable.

## Non-goals (explícitamente FUERA de scope — rechazar si un cambio apunta acá)
- NO reemplazar el sistema operativo (sí: arquitectura asistida / self-hosting gateado).
- NO multi-usuario por ahora (single-user asistente personal).
- NO ejecutar acciones del mundo real (dinero, comms, infra) sin gate humano.
- NO optimizar costo a costa de calidad/seguridad (barato ≠ degradar).
- NO crecer en complejidad sin que las métricas justifiquen (anti scope-creep).

## Métricas de éxito (6 meses)
1. Auto-mejora estructural: propone/implementa/valida capacidades en su propio código, zona verde, auditadas.
2. Reducción demostrable de intervención humana en tareas rutinarias.
3. Rollback automático probado: toda acción amarilla reversible y verificada.
4. Aprendizaje de políticas por calibración (bandit + ECE) sin intervención.
5. Integración autónoma de ≥1 fuente externa (pricing) → cambio verificado.
6. **Cero incidentes irreversibles**: ninguna acción zona-roja sin gate humano.

## Regla de alineación (la que aplica `goal_aligned()`)
Un cambio PASA solo si: **avanza el north star** Y **no viola ningún invariante** Y **no
toca un non-goal** Y **es reversible**. Ante la duda → refutar (skeptic default).
