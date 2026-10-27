# Root — GOAL (BORRADOR de Opus, 2026-10-27)

> Borrador para que el usuario lo lea. No tiene efecto hasta que el usuario lo copie a mano a `GOAL.md`
> y corra `root goal autorizar`. Ningún agente escribe `GOAL.md` ni `GOAL.hash`.
> Las líneas marcadas **[nuevo]** no estaban en el GOAL de mmorch. Salen de las decisiones del 2026-10-27.
> El usuario decide si quedan. Este bloque de aviso se borra al copiar.

> Contrato anti-drift. `root goal verificar` lo compara con `GOAL.hash` antes de cada corrida.
> Editarlo es zona roja: lo edita el humano y lo re-autoriza con `root goal autorizar`.

## Propósito
Root evalúa, rutea y aprende.
Root evalúa el valor de un proyecto: si funciona, y su seguridad, eficiencia, estructura, arquitectura y diseño.
Root rutea el trabajo entre el cupo de Claude y las APIs de DeepSeek y Anthropic, según lo que midió.
Root aprende de cada corrida: lo que registra cambia la próxima elección.

## Invariantes
- **Zona roja con gate humano:** dinero, claves, borrar datos fuera del sandbox, SO o red fuera del repo,
  comunicaciones en nombre del usuario, este GOAL y las políticas de seguridad.
- **Checkeable va a un oráculo**, nunca a un LLM.
- **El verificador razona y refuta por default**; el acuerdo no es confirmación. La familia es libre.
- **Nada se auto-aplica** sin rollback probado y una validación verde (tests, oráculo, costo y `root goal verificar`).
- **Costo acotado** por el balance de `root.toml`; barato no significa degradar la calidad medida.
- **Observabilidad:** toda corrida y toda auto-acción quedan en `eventos.jsonl` y `runs.jsonl`.
- **[nuevo] El evaluador se mide a sí mismo:** una dimensión entra al puntaje global solo si su calibración
  la respalda. Una dimensión sin calibrar se informa como "sin calibrar", nunca como un número.
- **[nuevo] Sin veredicto antes que un veredicto falso:** un error de infraestructura da "sin veredicto", nunca verde.
- **[nuevo] El builder nunca ve el oráculo oculto:** una corrida que lo lee queda inválida.
- **[nuevo] Toda intervención humana posterior al plan se cuenta.**

## Fuera de alcance
- Multi-usuario.
- Acciones del mundo real sin gate humano.
- Complejidad sin un criterio declarado de cuándo funciona.

## Regla de alineación
Un cambio pasa solo si sirve al propósito, no viola un invariante, no entra en lo que está fuera de
alcance y es reversible. Ante la duda, se refuta.
