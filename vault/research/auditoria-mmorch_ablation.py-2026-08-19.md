---
applies_to:
- orchestration
created: 2026-08-19
status: seed
tags:
- mmorch
- self-audit
title: auditoria mmorch/ablation.py 2026-08-19
---

El módulo de ablación es funcionalmente correcto pero tiene acoplamiento oculto con _SKEPTIC_SYSTEM, viola el principio de inyección de dependencias al hardcodear el parser, y tiene un edge case de división por cero no manejado.

## Findings (sobrevivieron refutacion 5/5 — 2 estructurales, 1 bugs, 1 de principios)

- **Acoplamiento oculto: _verify depende de _SKEPTIC_SYSTEM y _parse_verdict de patterns.py sin inyección** [alta/estructural]: Líneas 38-47: _verify importa _SKEPTIC_SYSTEM y _parse_verdict directamente de patterns.py. Si patterns.py cambia el formato del system prompt o el parser de veredictos, ablation.py se rompe silenciosamente sin error de importación. El principio de inyección de dependencias (seam + injection) exige que estos sean parámetros inyectables. Además, _SKEPTIC_SYSTEM es un nombre con prefijo _ (privado) que se está usando desde otro módulo — señal de que el acoplamiento no fue diseñado intencionalmente.
- **by_case almacena artifact truncado como label, duplicando datos que el llamador ya tiene** [baja/estructural]: Línea 66: `by_case.append({"label": c.label or c.artifact[:30], ...})` — si el llamador ya tiene los casos en memoria (los pasó como argumento), almacenar el artifact truncado en el resultado duplica información. El llamador puede reconstruir el label desde `cases[i].label` o `cases[i].artifact[:30]`. Esto viola la regla de 'estado en memoria, no releer disco' — aunque aquí es 'no duplicar en memoria'. El resultado debería referenciar el índice del caso, no copiar el dato.
- **División por cero en lat_avg cuando cases está vacío** [media/bug]: Línea 76: `lat_avg=round(sum(lats) / n, 2) if n else 0.0` — el guard `if n` protege la división, pero `sum(lats)` sobre una lista vacía devuelve 0.0, lo cual es correcto. Sin embargo, la línea 74 `accuracy=round(correct / n, 4) if n else 0.0` tiene el mismo patrón y es correcta. El bug real está en la línea 76: si `cases` está vacío, `n=0`, y aunque el guard evita el ZeroDivisionError, el resultado `0.0` es engañoso — debería ser `None` o un valor que indique 'no data', no 0.0 que sugiere 0% de precisión.
- **Violación del principio de inyección: call() no es inyectable, impidiendo mockear el provider en self-check** [media/principio]: Línea 42: `call(verifier_model, ...)` es una llamada directa al provider real. El principio de seams + injection dice que el payoff real de una interfaz es insertar un mock. Este módulo no tiene self-check (`__main__`) y no puede tenerlo sin refactorizar para inyectar `call`. Un self-check de ablación necesitaría mockear el provider para no hacer llamadas reales a la API — imposible con la estructura actual.
- **El ranking por accuracy desc, costo asc puede ocultar el trade-off cross-family** [baja/otro]: Línea 79: `configs.sort(key=lambda r: (-r.accuracy, r.cost_usd))` — este ordenamiento es correcto para encontrar el mejor verifier, pero el propósito del módulo es comparar cross-family vs same-family. El sort no agrupa por `cross_family`, así que el consumidor del resultado tiene que re-agrupar manualmente. Sería más útil devolver también una métrica agregada por familia (ej: accuracy promedio cross vs same) para que la comparación sea inmediata.
