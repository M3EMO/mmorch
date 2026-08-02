# Qué se babela y cuándo corre el ingest

Type: grilling
Status: open

## Question

Los gates ya deciden SI un babel paga (ratio+fidelidad). Falta decidir el
CUÁNDO/QUÉ intentar: ¿todo doc que entra al vault pasa por `babel.ingest()`
automáticamente (¿nightly? ¿al escribir?), o solo docs marcados? ¿Y el refresh
cuando el original cambia — hash del original en el frontmatter del babel +
barrido nocturno? Costo por intento ~10-15 llamadas baratas: dimensionar.
