# SDD fase 2, ticket 07 (qué es un requisito ubicuo): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo sabe el SDLC qué requisitos `R<n>` reciben un test de propiedades. Ticket: `.scratch/sdd-fase-2/issues/07-requisito-ubicuo.md`.

Insumos ya decididos: en modo área, cada requisito es un bloque con su regla, sus casos borde y su criterio (ticket 06); un test cubre un solo ID y cada ID tiene su archivo (ticket 05); la etapa 2b lee la spec fusionada y edita solo el delta (ticket 04).

## Summary / key decisions

- Marca explícita `[prop]` en el bloque del requisito (opción B); un regex la lee. EARS no se adopta para este fin (Q1).
- Criterio de la marca: regla que vale para un conjunto de entradas generable, con o sin disparador (Q2).
- La marca vale en los dos modos, con área y sin área (Q3).
- Formato `R3 [prop]: <regla>`; test aprobado con once casos (Q4). Ticket 07 resuelto.

## Q&A log

### Q1 — Cómo marca la spec un requisito con test de propiedades
- Asked: ¿EARS con detección por ausencia de disparador (A), marca `[prop]` (B), sección `## Propiedades` (C) o todos candidatos (D)?
- Captured: el usuario pregunta "Conviene adoptar EARS? Si no, B". Claude responde: EARS ordena la redacción; la marca elige qué requisito recibe propiedades; para elegir, EARS es frágil (un caso borde con "Si" confunde la detección por ausencia) y ninguna medición muestra que EARS mejore el código de un agente (nota del vault sobre SDD). El usuario queda en **B**.
- Captured: EARS queda como mejora posible de la plantilla, medida aparte; anotado en "Not yet specified" del mapa.
- Flags: ninguno.

### Q2 — Qué requisito merece la marca
- Asked: ¿solo ubicuos sin disparador (A), cualquier regla sobre un conjunto de entradas generable con o sin disparador (B), o criterio libre del writer (C)?
- Captured: el usuario elige **B**. La marca dice "esta regla vale para muchas entradas", que es lo que prueba Hypothesis. El criterio queda escrito en la plantilla; el writer marca en la etapa 2 y la etapa 2b aplica el criterio al revisar.
- Flags: si la marca vale también sin área (Q3).

### Q3 — La marca sin área
- Asked: ¿la marca vale con área y sin área (A) o solo en modo área (B)?
- Captured: el usuario elige **A**. Las propiedades no dependen de los deltas; los repos sin áreas son hoy la mayoría; la plantilla es la misma; una spec sin marcas sigue igual que hoy.
- Flags: ninguno.

### Q4 — Test de aceptación y formato de la marca
- Asked: ¿marca pegada al ID (`R3 [prop]: <regla>`) y test de ocho casos? Después: ¿falta algo?
- Captured: el usuario aprueba el formato y once casos: los ocho más un `ADDED` con marca queda marcado al fusionar, un `MODIFIED` que solo agrega o quita la marca pasa el gate del ticket 04 (no choca con su caso 15), y la spec base de la etapa 0 puede llevar marcas.
- Riesgo sin test: un writer que marca todo; lo atrapa el ticket 09 (propiedad vacua no mata mutantes).
- Flags: ninguno. Ticket 07 resuelto.

## Open flags (pending input)
