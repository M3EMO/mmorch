# Gate de Cursor: informe después de la primera edición
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿El informe sigue evitando el error del lector acoplado cuando llega DESPUÉS de la primera edición del archivo, como permite el hook `postToolUse` de Cursor?

Protocolo: el banco del ticket 09 (`../orch-spike/scripts/tareas/`) con una condición C: el informe se inyecta como observación después del primer `write_file` sobre el archivo escritor, no al inicio. Mismo criterio: C supera a A por al menos 25 puntos en acopladas sin perder más de 10 en control. Si pasa, se instala el hook en `~/.cursor/hooks.json` (decisión del ticket 03).

## Comments

**2026-09-25 — pre-registro (Claude, antes de correr):** condición C = el informe (mismo extractor del ticket 09) se agrega a la observación del primer `write_file` sobre el archivo escritor, una sola vez. Modelo principal deepseek-v4-pro, 12 tareas x 3 repeticiones. Se compara contra la condición A ya medida en el ticket 09 (mismo modelo, mismas tareas, mismo agente). Criterio: C - A >= 25 puntos en acopladas y C no pierde más de 10 puntos en control.

## Answer

Aprobado (2026-09-25). Condición C (informe después del primer `write_file` sobre el escritor), deepseek-v4-pro: acopladas 79% (19/24) contra A 42% (10/24), +37 puntos, p = 0.0086 (Fisher, una cola); control 100% en ambas; costo US$0.36. Cumple el criterio (+25 sin perder control). C rinde menos que B (96%): en las 5 fallas el agente vio el informe después de escribir y terminó sin tocar al lector. Estas corridas usaron el extractor del ticket 09; el de `mmorch/impacto.py` es menos ruidoso. Siguiente: modo Cursor del módulo y hook `postToolUse` en `~/.cursor/hooks.json`, con OK del usuario. Aceptado por el usuario el 2026-09-25.
