---
applies_to:
- orchestration
created: 2026-08-21
status: seed
tags:
- mmorch
- self-audit
title: auditoria mmorch/arbitration.py 2026-08-21
---

El módulo arbitration.py es un registro auditable de arbitrajes con API pequeña y cohesión alta. Se identificaron 4 hallazgos: un bug de truncamiento silencioso de evidencia, una violación estructural al releer el archivo del disco cuando el llamador ya tiene los datos en memoria, una violación de principio por acoplamiento oculto con el orquestador, y un hallazgo de otro tipo sobre la pérdida de datos por truncamiento de razones.

## Findings (sobrevivieron refutacion 4/4 — 1 estructurales, 1 bugs, 1 de principios)

- **Relectura de disco en pending_recheck() cuando el llamador ya tiene los datos** [media/estructural]: pending_recheck() llama a _read(path) que relee el archivo JSONL completo del disco. Pero el orquestador que llama a esta función típicamente ya tiene los registros en memoria (p.ej. después de llamar a stats() o a log()). Esto viola el ADR 0001 (estado en memoria, no releer disco). El módulo debería aceptar una lista de registros como parámetro opcional (p.ej. rows=) para que el llamador pueda pasar los datos que ya tiene, evitando I/O redundante. La firma actual fuerza una relectura innecesaria cada vez que se consulta la cola de re-check.
- **Truncamiento silencioso de evidencia en log()** [media/bug]: En log(), el parámetro evidence se trunca a 200 caracteres (evidence[:200]) sin advertencia. Si el orquestador pasa una evidencia más larga (p.ej. un probe con salida extensa), el registro guarda una versión incompleta. Luego, stats() calcula 'dismissed_without_evidence' basándose en si el campo está vacío, pero un registro con evidencia truncada a 200 chars sigue contando como 'con evidencia'. Esto puede inflar la tasa de descartes con evidencia y ocultar descartes que en realidad tenían evidencia insuficiente pero que fue cortada. El truncamiento debería al menos registrar un flag o el largo original.
- **Acoplamiento oculto: el orquestador debe conocer el formato interno de los registros** [baja/principio]: El docstring dice 'El re-check es juicio del orquestador; acá solo se surfacea la cola'. Pero pending_recheck() devuelve dicts crudos con campos internos (ts, critique, verdict, reason, source, evidence, rechecked). El orquestador que hace el re-check debe saber que la clave 'evidence' existe y qué significa, y debe mutar el dict para marcar 'rechecked': True. Esto es un acoplamiento oculto: si el módulo cambia el formato interno (p.ej. renombra 'evidence' a 'proof'), el orquestador se rompe silenciosamente. El módulo debería exponer una función mark_rechecked(rec) o devolver objetos con métodos, no dicts crudos.
- **Pérdida de información por truncamiento de reason en log()** [baja/otro]: El parámetro reason se trunca a 400 caracteres (reason[:400]). Para un registro de auditoría, la razón del veredicto es el dato más valioso — es lo que permite auditar si el árbitro decidió bien. Truncarla a 400 chars puede perder el matiz de la decisión. Si el orquestador pasa una razón larga (p.ej. citando código o contexto), la auditoría queda incompleta. A diferencia de evidence (que es un campo auxiliar), reason es el corazón del registro. Considerar no truncar reason, o al menos guardar el largo original para detectar truncamientos.
