---
title: Planner de SDLC sobre corridas reales: Sonnet y Cursor no superan al reasoner
created: 2026-10-02
tags: [research, orchestration, sdlc, planner, cursor, sonnet, reasoner, banco, corridas-reales]
status: measured
confidence: media: 12 corridas reales x 2 repeticiones; potencia baja para diferencias chicas
sources: [bd orchestration-34k, orch-spike/scripts/sdlc_planner (7058afe congelado, fc933ae resultados), mmorch/sdlc.py (e6f358b)]
---
## Pregunta

¿Qué planner conviene en la etapa 3-plan de SDLC? Brazos: el reasoner actual (`deepseek-reasoner`, una llamada desde la spec, sin ver el repo), Sonnet como agente de solo lectura (`claude -p` con Read/Grep/Glob) y Cursor `grok-4.7-medium` en `--mode plan`. Sigue al banco sintético del ticket 06 de `cursor-trabajador`, donde Sonnet planeó 24/24 tareas acopladas y el reasoner 15/24.

## Diseño

- Oráculo: repetir corridas reales desde su spec, con el mismo test de aceptación, y contar `ok` = la aceptación pasa sin escalar a Claude.
- Comparar el plan con el diff final era circular: `gate_alcance` impide tocar archivos fuera del plan, y en 25 de 28 corridas el diff coincidía con el plan del reasoner.
- Ítems: 12 corridas repetibles (9 de SSB, 1 de Estudio, 2 de ChatBot). Portfolio quedó afuera porque Cursor no puede ver su código.
- Dos repeticiones por ítem y brazo; etapas 3 a 5 con el coder `deepseek-v4-pro`; la escalada a Claude reemplazada por un comando vacío.
- Regla pre-registrada: un brazo gana si mejora la tasa por ítem en al menos 0.15 con cota inferior bootstrap al 90% mayor que 0.
- Una revisión adversarial confirmó 9 defectos antes de congelar. El más grave: el coder no veía el test de aceptación, y eso favorecía a los planners que leen el repo.

## Resultado

| Brazo | Éxito por ítem | d contra el reasoner | Escaladas | USD DeepSeek | Rechazos de gates | Minutos (mediana) |
|---|---|---|---|---|---|---|
| reasoner | 0.875 | — | 3 | 5.26 | 29 | 8.2 |
| Sonnet | 0.917 | +0.042 (cota −0.042) | 2 | 5.02 | 21 | 8.4 |
| Cursor | 0.958 | +0.083 (cota 0.000) | 1 | 3.71 | 15 | 11.4 |

- Veredicto: ningún brazo gana; el reasoner sigue como planner de SDLC.
- Sonnet gastó US$1.23 equivalentes de cupo de Claude en 24 planes. Cursor usó 1.7M tokens de su pool.
- Desvío declarado: las 6 corridas de Estudio fallaron primero por un error del arnés ("Filename too long" en el clon). Se repitieron con `core.longpaths` en los tres brazos y pasaron todas; el veredicto es el mismo con y sin el desvío.

## Lectura

- En corridas reales la diferencia entre planners se achica mucho frente al banco sintético. Las features reales tocan 1 a 3 archivos, y la spec revisada ya nombra los archivos: planear es fácil.
- El cuello de botella es el coder: las fallas son de compilación en la etapa 4 en los tres brazos.
- Las métricas secundarias favorecen a Cursor (menos escaladas, menos rechazos, menos USD), pero no alcanzan el umbral y Cursor tarda más.
- El cambio en `mmorch/sdlc.py` queda: `SDLC_PLANNER` permite elegir el planner sin tocar el default.
