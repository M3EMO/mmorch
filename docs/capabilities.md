# Capacidades — cuándo elegir

Directiva de turno: `CLAUDE.md`. Invariantes: `GOAL.md`. Qué existe ahora: `docs/generated/catalog.md`. Modelos y precios: `mmorch/config.py`. Este archivo no cataloga ni cuenta.

## Patrones

Elegir por forma del trabajo, no por “usar más agentes”.

- `fan_out` — N subtareas independientes; paralelo en el nodo barato. Síntesis la hace el conductor.
- `adversarial_verify` — un artefacto vs rúbrica. Subjetivo: familias distintas (OneFlow). Checkeable: `checker=` en código, no un LLM.
- `route` — el barato responde con self-score; escalar al conductor solo si la confianza es baja.
- `cascade` — varios umbrales de esfuerzo; el bandit aprende el umbral. Distinto de `route` (un salto vs una escalera).
- `ensemble_verify` — K escépticos cross-family, mayoría; empate = falla. No repetir el mismo modelo (n_eff no sube).
- `tournament` — best-of-N pairwise; juez de otra familia; empate → conductor.
- `bucket_rank` — set grande a tiers, O(n); no se pierden ítems. No es ranking total.
- `loop_until_done` — el alcance no se conoce; seguir hasta que el lote salga vacío. Discovery, no métrica.
- `classify_and_act` — puerta de entrada: clasificar barato y disparar handler, o escalar si la clase/confianza no alcanza.
- `hillclimb` — hay métrica escalar corrible (`score` = checker o comando, nunca LLM-judge). Distinto de `loop_until_done`.
- `generate-and-filter` — no es patrón propio: se arma con los de arriba.

## Schema-gate

`gated_json` valida-o-rechaza (library, opt-in). No está forzado en `adversarial_verify`: ahí unparse → failed es más seguro que una excepción.

## Feedback

El label es un outcome de ejecución (`record_outcome` / bandit), no la confianza auto-reportada. Detalle de implementación: `mmorch/feedback.py`.
