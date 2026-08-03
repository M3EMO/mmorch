# Qué se babela y cuándo corre el ingest

Type: grilling
Status: resolved

## Question

Los gates ya deciden SI un babel paga (ratio+fidelidad). Falta decidir el
CUÁNDO/QUÉ intentar: ¿todo doc que entra al vault pasa por `babel.ingest()`
automáticamente (¿nightly? ¿al escribir?), o solo docs marcados? ¿Y el refresh
cuando el original cambia — hash del original en el frontmatter del babel +
barrido nocturno? Costo por intento ~10-15 llamadas baratas: dimensionar.

## Answer

Grilling 2026-08-03. El timing (async al escribir + nightly red de seguridad) y el
refresh (hash del original en frontmatter del babel) ya quedaron decididos en el 03 —
este ticket cerró solo el alcance:

**Automático con pre-filtro determinista gratis**: se intenta babel sobre todo doc que
entra al vault QUE cumpla: >= 3000 chars · fuera de `archive/` · fuera de infra
(lexicon, README, templates, moc/). Sin marcado manual. Los gates de ejecución
(ratio <= 0.7, fidelidad >= 0.8) deciden lo demás; un rechazo por ratio cuesta solo
el encode (~3-5 llamadas flash-lite, centavos).
