# Checkpoints de resumabilidad best-effort sin señal al fallar

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: S
Eje: robustez
Evidence: mmorch/server_engine.py:43-48,53-56,155,343-344,384-385

Si workflow_store falla, el job sigue (deliberado) pero el checkpoint no se persiste y
nadie se entera: un resume posterior re-arranca de step 0 re-pagando pasos. `emit()` ya
existe en el archivo.

**Fix:** `emit("job","warn",...)` en los except de checkpoint/spec.

## Comments
Agregado `emit("job"/"step", "warn", ...)` con el detalle de la excepción (truncado a
150 chars) en los 5 except de checkpoint/spec citados (líneas 43-48, 53-56, ~155,
343-344, 384-385 de la versión pre-fix); el job sigue corriendo (deliberado, best-effort)
pero ahora queda señal en el event bus. No hay test nuevo dedicado (nice-to-have, S,
verificado leyendo el diff — server_engine.py no tiene suite propia hoy).
