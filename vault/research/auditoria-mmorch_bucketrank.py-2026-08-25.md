---
title: auditoria mmorch/bucketrank.py 2026-08-25
status: seed
tags: [mmorch, self-audit]
created: 2026-08-25
---

El módulo bucketrank.py implementa clasificación paralela de items en tiers con manejo conservador de fallos. Se identificaron 4 hallazgos: un bug en el manejo de excepciones del pool, una violación estructural del principio de estado en memoria, una violación de principio por acoplamiento oculto al formato de respuesta, y un hallazgo menor de robustez en el parsing de tiers.

## Findings (sobrevivieron refutacion 4/4 — 1 estructurales, 1 bugs, 1 de principios)

- **Re-lectura de items desde disco cuando el llamador ya los tiene en memoria** [media/estructural]: El módulo recibe `items: list[str]` como parámetro, pero el docstring dice 'graduar/ordenar un set GRANDE'. Si el llamador ya tiene los items en memoria (ej: resultado de una query), pasarlos como lista es correcto. Sin embargo, el patrón de diseño sugiere que podría releerse de disco en otros módulos. Este módulo en sí no viola ADR-0001, pero su interfaz `items: list[str]` fuerza al llamador a materializar todo en memoria, contradiciendo el espíritu de 'estado en memoria' para sets grandes. El llamador podría tener que releer de disco para construir la lista.
- **Excepción en f.result() aborta todo el proceso sin registrar fallo** [alta/bug]: En el bucle `for f in as_completed(futs)`, si una tarea lanza una excepción no capturada por `_job` (ej: error en `f.result()` por cancelación o error interno del executor), el `except Exception` en `_job` no la cubre. Esto aborta `bucket_rank` completo, perdiendo todos los resultados ya procesados. El diseño dice 'Alineacion item<->tier preservada aunque una llamada falle' pero esto solo aplica a fallos dentro de `_grade_one`, no a fallos del pool. Debería capturarse en el bucle de `as_completed` y tratar como `None`.
- **Acoplamiento oculto al formato exacto de respuesta del modelo** [media/principio]: El prompt exige 'exactamente: TIER: <uno de ...>' y `_extract_tier` usa regex `TIER\s*[:=]\s*([A-Za-z0-9]+)`. Si el modelo responde con formato ligeramente diferente (ej: 'Tier: S' con mayúscula, o 'TIER = S' con espacios), el regex falla y cae al tier más bajo silenciosamente. Esto viola el principio de 'seams + injection' — no hay forma de inyectar un parser alternativo. El módulo asume un contrato frágil con el modelo sin exponer un seam para adaptarse a variaciones.
- **Resultados no ordenados dentro de cada tier** [baja/otro]: El docstring dice 'graduar/ordenar un set GRANDE en tiers' pero `by_tier` mantiene el orden de llegada de `as_completed`, que es no-determinístico. Si el llamador espera un orden estable dentro del tier (ej: por calidad), no lo obtiene. Aunque no es un bug funcional, la palabra 'ordenar' en el docstring es engañosa — el módulo solo clasifica, no ordena dentro del tier.
