---
title: Cursor como explorador o borrador para Opus no ahorra cupo; Sonnet es el mejor planner sin Opus
created: 2026-10-01
tags: [research, orchestration, cursor, delegacion, cupo, planner, sdlc, anclaje, banco]
status: measured
confidence: "media: 50 preguntas y 12 tareas sintéticas; el rol B es un piloto"
sources: [.scratch/cursor-trabajador/issues/06-banco-roles-explorador-y-borrador.md, .scratch/cursor-trabajador/research/06-preregistro.md, orch-spike/scripts/cursor_roles (fcb7ea4)]
---
## Pregunta

¿Cursor (`cursor-agent -p`, `grok-4.7-medium`) como explorador del repo o como borrador de plan ahorra cupo de Opus sin bajar la calidad de lo que Opus decide? Ticket 06 del mapa wayfinder `cursor-trabajador`, banco pre-registrado y congelado antes de gastar Opus.

## Diseño

- Rol A, explorador: 50 preguntas sobre el paquete mmorch (20 de ubicación, 30 de llamadores con verdad de traza en runtime). Brazos: Opus solo, Opus con el resumen de Cursor, Opus con un resumen con un defecto plantado.
- Rol B, borrador de plan: piloto con 12 tareas acopladas sintéticas. El oráculo compara los archivos del plan con los de la solución correcta; un implementador DeepSeek restringido al plan corre los tests ocultos.
- Cupo: `total_cost_usd` de `claude -p`, menos el piso de una corrida vacía. Umbral: ahorro de 30% con cota superior del cociente 0.85; no inferioridad con margen 0.10.

## Resultados

- Explorador: NO ADOPTAR. Calidad 0.967 sin resumen y 0.979 con resumen. Ahorro 14.7% (cociente 0.853, cota 0.921). La mirada intermedia cortó por futilidad a los 25 ítems (ahorro 12.6%).
- Borrador para Opus: NO ADOPTAR. Opus planea 12/12 con y sin borrador, y con borrador gasta 5.7% más.
- Anclaje: Opus no adoptó ninguno de los 28 defectos plantados. Verifica el borrador abriendo los archivos citados.
- Causa: Opus resuelve una búsqueda con 4 o 5 llamadas baratas (US$0.06-0.08). Un resumen ahorra poco cuando explorar ya cuesta poco.

## Planners sin Opus (12 tareas × 3)

| Planner | Acopladas con todos los lectores | Tests ocultos con el implementador | Recurso por plan |
|---|---|---|---|
| Sonnet | 24/24 | 1.000 | US$0.034 equivalentes de cupo de Claude |
| Cursor grok | 19/24 | 0.861 | pool de Cursor |
| Haiku | 18/24 | 0.778 | US$0.017 equivalentes de cupo de Claude |
| DeepSeek v4 pro | 17/24 | 0.778 | US$0.006 de API |
| deepseek-reasoner (exploratorio) | 15/24 | 0.750 | US$0.0007 de API |

Todos los planners que fallan omiten a los lectores y planean solo el escritor. En las tareas de control, los cinco aciertan todo.

## Hallazgos laterales

- `semSearch` no existe en `cursor-agent -p`.
- Sin interfaz y con `--force`, Cursor ve las herramientas de Gmail, Calendar y Drive de sus plugins. `deny: ["Mcp(*:*)"]` en `<workspace>/.cursor/cli.json` las bloquea.
- `--mode plan` deja el plan en la herramienta `createPlan`, no en `result`.
- `deepseek-reasoner` en un bucle de herramientas JSON pone `summary` fuera de `args`.

## Consecuencia

Cursor rinde como implementador (ticket 04), no como ayudante de exploración o planificación para Opus. Para la etapa de plan de SDLC, donde no hay Opus, Sonnet es el mejor planner medido; Cursor es la opción sin costo. Falta medirlo sobre corridas reales de SDLC, que hoy planean con una sola llamada del reasoner desde la spec.
