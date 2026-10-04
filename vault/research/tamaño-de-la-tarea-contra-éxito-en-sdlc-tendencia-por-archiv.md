---
title: "Tamaño de la tarea contra éxito en SDLC: tendencia por archivos, sin datos para escalones grandes"
created: 2026-10-04
tags: [research, orchestration, sdlc, tamano, gate, plan, carga-cuantizada]
status: measured
confidence: "baja: 107 corridas, las 72 del banco vienen de 12 tareas"
sources: [bd orchestration-e4d, orch-spike/scripts/sdlc_tamano (e16590b), mmorch/sdlc.py gate_tamano (85c3de8), informe de impacto (3114940), orch-spike/scripts/sdlc_planner/pares_e4d.py (8eb8218)]
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

## Actualización (2026-10-04): qué predice el fracaso

AUROC para predecir el fracaso en 98 corridas con las tres medidas (15 fallas, bootstrap por tarea, 38 grupos):

| Medida | AUROC | IC 90% |
|---|---|---|
| Archivos del plan | 0.651 | 0.529 – 0.769 |
| Tokens de archivos existentes | 0.525 | 0.383 – 0.648 |
| Acoplamiento (lectores por literales fuera del plan) | 0.500 | 0.365 – 0.624 |

- Solo los archivos predicen algo, y poco. El acoplamiento por literales no predice: no se rutea por esa medida.
- Igual se agregó el informe de impacto al planner y al coder, por la evidencia del banco sintético del mapa de impacto.

## Validación del informe de impacto en corridas reales (2026-10-04)

Brazo reasoner con el informe (3114940) contra la línea base del banco 34k, 24 pares emparejados por tarea y repetición (12 tareas). Script: `orch-spike/scripts/sdlc_planner/pares_e4d.py` (8eb8218).

| Medida | Línea base (2026-10-02) | Con informe (2026-10-04) |
|---|---|---|
| Éxito sin Claude | 21/24 | 24/24 |
| Escaladas a Claude | 3 | 0 |
| USD total | 5.26 | 4.15 |
| Tiempo mediano por corrida | 496 s | 996 s |

- Los 3 pares que cambian van de escalada a construido. Ningún par empeora.
- La diferencia de éxito es +0.125, con IC 90% por tarea de +0.04 a +0.25.
- McNemar exacto con 3 pares discordantes da p = 0.25 a dos colas: la señal es positiva, pero chica.
- Los brazos corrieron en días distintos, sin intercalar: la deriva del proveedor no queda controlada.
- El USD total baja porque una corrida escalada gasta la escalera completa. La mediana por par cambia −0.003 USD.

### De dónde sale el tiempo extra

- Las llamadas a la API suman unos 150 s por corrida en los dos brazos, a 117 tokens/s.
- El informe tarda menos de 0.5 s por archivo, también con la caché fría.
- Los dos brazos planean la misma cantidad de archivos (mediana 2 contra 1).
- La etapa 5 domina cada corrida: `gate_mutacion` corre el comando de aceptación una vez por mutante.
- En repos que no son Python, ese comando corre la suite entera (por ejemplo, `mvn test`).
- Ningún commit cambió la etapa 5 entre las dos fechas.
- En el chatbot, cada mutante tarda 115 s hoy y tardaba 75 s el 2026-10-02.
- La causa más probable es la carga de la máquina, sin prueba directa. El informe no explica el tiempo.

## Decisión sobre el informe

- El informe de impacto queda en el planner y en el coder.
- La etapa 5 es el costo de tiempo dominante del pipeline. Paralelizar los mutantes es una mejora candidata, sin medir.
