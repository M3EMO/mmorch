# mmorch — GOAL

> Contrato anti-drift. `goal_aligned()` lo lee antes de auto-aplicar un cambio. Editarlo es zona roja:
> lo edita el humano y lo re-autoriza con `authorize_goal()`.

## Propósito
mmorch mide y delega. Mide, sobre tareas con etiqueta computable, qué configuración de modelos acierta
y cuánto cuesta. Con esas mediciones rutea el trabajo recurrente a APIs baratas y libera cupo de Claude.

## Invariantes
- **Zona roja con gate humano:** dinero, claves, borrar datos fuera del sandbox, SO o red fuera del repo,
  comunicaciones en nombre del usuario, este GOAL y las políticas de seguridad.
- **Checkeable va a un oráculo**, nunca a un LLM.
- **El verificador razona y refuta por default**; el acuerdo no es confirmación. La familia es libre.
- **Nada se auto-aplica** sin rollback probado y `fitness()` verde (tests, checkers, costo, `goal_aligned`).
- **Costo acotado** por el BudgetKeeper; barato no significa degradar la calidad medida.
- **Observabilidad:** toda auto-acción queda en un episodio auditable.

## Fuera de alcance
- Multi-usuario.
- Acciones del mundo real sin gate humano.
- Complejidad que ninguna métrica justifica.

## Regla de alineación
Un cambio pasa solo si sirve al propósito, no viola un invariante, no entra en lo que está fuera de
alcance y es reversible. Ante la duda, se refuta.