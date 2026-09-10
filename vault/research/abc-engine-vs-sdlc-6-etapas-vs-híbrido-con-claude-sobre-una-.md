---
title: A/B/C engine vs SDLC 6 etapas vs híbrido con Claude sobre una feature real — 2026-09-10
created: 2026-09-10
tags: [research, mmorch, sdlc, gates, project-build, ab-test, synth, escalacion, quetepario-chatbot]
status: validated
confidence: alta en lo medido (una feature, un repo); baja para generalizar sin las 3 features del ticket 06
sources: [docs/ab-sdlc-2026-09-10/README.md, docs/ab-sdlc-2026-09-10/ab_results.json, logs/metrics.jsonl (fases ab-sdlc-B / ab-sdlc-C), commits 58ce334 ea8b775, ramas ChatBot ab/b-sdlc f9798ad y ab/c-hybrid dcdef5c]
---
## Pregunta
¿Un SDLC de 6 etapas con gates le gana al engine actual de /project en una feature real? ¿Cuánto agrega Claude supervisando los gates? Medición go/no-go del goal "SDLC 6 etapas".

## Diseño
Feature: port del matcher demo.py (QueTePario/ChatBot) a Java 17, 35 asserts de aceptación traducidos 1:1 del test Python (oráculo). Baseline compartido d6853c9. Tres brazos: A = /project engine (server); B = 6 etapas scriptadas (intent humano, spec+plan reasoner, build v4-pro, gates deterministas, fix loop); C = igual que B pero Claude decide en los gates y no hay fix automático.

## Resultado
| brazo | aceptación | llamadas | USD | min | intervenciones |
|---|---|---|---|---|---|
| A engine | rojo, 5 intentos | 11-21 | 0.81 | ~120 | 4 fixes de engine |
| B script | verde (v2) | 31 | 1.13 (2.68 con v1) | 22 | 0 humanas, 2 de arnés |
| C híbrido | verde | 9 | 0.16 | 9 | 2 de Claude, 4 min |

## Lo que se aprendió (por orden de peso)
1. **Lo que pasa tras el primer rojo decide todo.** B y C compartían spec, plan y coder. B v1 reescribía 9 archivos por vuelta y se rompió a sí mismo. B v2 (el modelo nombra archivos + compile gate con revert) dio verde en 2 vueltas. C: Claude leyó el error y corrigió 4 líneas.
2. **Las intervenciones de Claude eran gates que faltaban.** I1 = revisión de plan (2 regex sobre plan.md). I2 = `mvn test-compile` (3 s, $0). Con esos gates, C habría entrado 0 veces. Regla: cada intervención deja una corrección Y un chequeo nuevo.
3. **El engine de /project tenía 4 defectos, todos invisibles hasta correrlo sobre algo real**: planner que convierte el test de aceptación en unidad; coder ciego a sus deps; deepseek-chat + max_fix=1 por un regex; max_tokens 16384 vacía la salida de v4-pro en archivos de 500 líneas. Quedan 4 más (recursión sin gen_model, max_depth de la forma VERIFY, sin test-compile, integración no realimenta).
4. **A terminó a un método de verde y no tenía forma de saberlo.** Sin gate de test-compile ni loop sobre la integración, escala y para. Un pipeline que no puede decir "estás a 4 líneas" desperdicia el trabajo hecho.
5. **Costo**: la supervisión bien puesta es más barata que la automatización ciega: C $0.16 vs B $1.13, y la mitad del tiempo. Pero C cuesta cupo de Claude (4 min) que no está en dólares.
6. Ningún brazo tuvo tests por módulo. Solo el total. La corrupción por integración no se detecta por unidad (ticket 13 del mapa).

## Decisión
GO para el mapa wayfinder `.scratch/sdlc-6-gates/` (13 tickets). El pipeline reemplaza al engine de /project y se convierte en convención por repo (re-scope del usuario: arquitectura de proyectos, no feature de Lotus). Validación pendiente: 3 features reales + 2 sintéticas (tickets 06/07).

## Lecciones de arnés
- Un proceso hijo de la sesión de Claude Code muere con ella: server y drivers van desacoplados (Start-Process).
- El worklist cacheado por task hace que "relanzar" sea byte a byte igual: hay que borrar la entrada o cambiar la task.
- Otra sesión movió `main` del ChatBot durante el experimento; el brazo A se fijó a un worktree en el baseline.
