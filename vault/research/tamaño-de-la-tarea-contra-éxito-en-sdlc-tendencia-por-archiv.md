---
title: "Tamaño de la tarea contra éxito en SDLC: tendencia por archivos, sin datos para escalones grandes"
created: 2026-10-04
tags: [research, orchestration, sdlc, tamano, gate, plan, carga-cuantizada]
status: measured
confidence: "baja: 107 corridas, las 72 del banco vienen de 12 tareas"
sources: [bd orchestration-e4d, orch-spike/scripts/sdlc_tamano (e16590b), mmorch/sdlc.py gate_tamano (85c3de8)]
---
## Pregunta

¿Conviene cuantizar las tareas de SDLC en escalones exponenciales (×4 en tokens de contexto) y frenar en la etapa 3 las tareas grandes?

## Datos (2026-10-04)

107 corridas: 35 run-logs históricos y 72 repeticiones del banco de planners (orchestration-34k). Tamaño = archivos del plan y tokens (bytes/4) de los archivos del plan que ya existen en la base.

| Archivos del plan | Corridas | Éxito sin Claude | Llamadas (mediana) |
|---|---|---|---|
| 1 | 53 | 0.89 | 3 |
| 2-3 | 36 | 0.81 | 5 |
| 4-7 | 17 | 0.65 | 7 |
| 8+ | 1 | 1.00 | 11 |

- En tokens, 83 corridas tienen hasta 4k y 15 entre 4k y 16k. Ninguna pasa de 16k, porque casi todas crean archivos nuevos.
- Los escalones de 64k y más del ticket no tienen datos para calibrarse.
- Las 72 repeticiones vienen de 12 tareas: no son independientes.

## Decisión

- La etapa 3 registra el tamaño y el escalón de cada plan en el run-log (`plan_tamano`).
- Una baranda frena el plan solo fuera del rango observado: más de 8 archivos o más de 16k tokens. Pide dividir la tarea y no intenta reescribir el plan.
- Los umbrales se configuran por repo (`tamano_max_archivos`, `tamano_max_tokens` en `sdlc.toml`).
- No hay evidencia de que dividir una tarea de 4 a 7 archivos mejore el éxito: los umbrales se revisan con 20 corridas de 4 o más archivos, o con un experimento de división.
