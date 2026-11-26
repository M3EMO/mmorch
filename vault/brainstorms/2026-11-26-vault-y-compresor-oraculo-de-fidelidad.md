---
title: "Vault y compresor: oráculo de fidelidad (grilling)"
created: 2026-11-26
tags: [brainstorm, orchestration, vault-y-compresor]
status: seed
---
# Vault y compresor: notas del grilling del oráculo de fidelidad
Fecha: 2026-11-26 · Objetivo: resolver el ticket "Oráculo de fidelidad del compresor" (`.scratch/vault-y-compresor/issues/06-oraculo-de-fidelidad.md`).

## Resumen y decisiones

- El oráculo tiene dos piezas: una batería de preguntas con respuesta exacta y un gate final de tareas reales. Ninguna pieza usa un LLM-juez.
- La batería corre en cada iteración del compresor. Haiku 5.5 la responde por API, sin cupo, y compara el original con el comprimido.
- El gate de tareas corre una vez antes de aceptar, con el modelo real en `claude -p`.
- Piso: la batería retiene 0.965 o más, el subconjunto crítico retiene el 100% y las tareas no admiten regresiones.
- Tamaño: 200 casos (120 de ruteo y 80 de reglas, 20 de ellos críticos), mayoría de 3 corridas, y 10 tareas.
- El gold sale de transcripts reales, y el usuario aprueba las preguntas de reglas y elige las tareas.
- El oráculo y su gold viven en Root, junto al compresor (sale del ticket 01).

## Registro de preguntas

### P1 — Qué mide el oráculo
- Pregunta: ¿batería de preguntas con respuesta exacta, solo ruteo, contención de hechos o tareas reales?
- Respuesta del usuario: "1 y 4".
- Captura: el oráculo combina dos piezas. La primera es la batería de preguntas con respuesta exacta: ruteo (pedido real → nombre del skill) y reglas (dato del CLAUDE.md → respuesta exacta). La segunda es una batería de tareas reales. Ninguna pieza usa un LLM-juez.
- Descartado: la contención de hechos, porque exige texto literal o un juez de implicación.
- Pendiente: el rol y el checker de las tareas reales → P2.

### P2 — Rol de las tareas reales
- Pregunta: ¿las tareas son un gate final, corren en cada iteración o corren aparte como alarma?
- Respuesta del usuario: "Gate final".
- Captura: la batería exacta corre en cada iteración del compresor. Las tareas reales corren una vez, antes de aceptar una versión comprimida, con pocos casos. Un checker determinista juzga cada tarea: un test que pasa, un archivo creado o el skill invocado en el transcript.

### P3 — Qué modelo responde cada pieza
- Pregunta: ¿Haiku para la batería y el modelo real para las tareas, el modelo real en todo, o Haiku en todo?
- Respuesta del usuario: "Haiku + real".
- Captura: Haiku 5.5 responde la batería por API, en dólares y sin cupo. La medida es relativa: el original y el comprimido corren con el mismo modelo. El gate final de tareas corre con el modelo real en `claude -p` y gasta poco cupo una sola vez.
- Apoyo: SkillReducer midió la retención 0.965 en 5 modelos de 4 familias.

### P4 — El piso
- Pregunta: ¿0.965 con un subconjunto crítico al 100%, 0.965 parejo o 0.99?
- Respuesta del usuario: "0.965 + 100% crítico".
- Captura: la retención es igual a los aciertos del comprimido divididos por los aciertos del original, sobre los casos que el original acierta. La batería completa retiene 0.965 o más. Las preguntas de guardrails y de zona roja retienen el 100%. En el gate de tareas, el comprimido pasa toda tarea que el original pasa.

### P5 — Cuántos casos
- Pregunta: ¿200 casos y 10 tareas, 100 y 5, o 400 y 20?
- Respuesta del usuario: "200 + 10 tareas".
- Captura: la batería tiene 200 casos. Hay 120 de ruteo, y unos 25 de ellos esperan "ningún skill" para detectar disparos en falso. Hay 80 de reglas, y 20 de ellos son críticos. Cada caso corre 3 veces y vale la mayoría. El gate final tiene 10 tareas reales.
- Aritmética: con 200 casos que el original acierta, el piso de 0.965 admite como máximo 7 fallas.

### P6 — De dónde sale el gold
- Pregunta: ¿transcripts con aprobación humana, todo a mano o todo generado?
- Respuesta del usuario: "Transcripts + aprobación".
- Captura: los casos de ruteo salen de transcripts reales: el pedido y el skill invocado de verdad. Los casos "ningún skill" salen de pedidos reales sin invocación. Un LLM redacta las preguntas de reglas desde los archivos originales, y el usuario aprueba cada una. El usuario elige las 10 tareas entre candidatas de transcripts con resultado checkeable. Un caso entra en la batería solo si el original lo acierta en 2 de 3 corridas.

### P7 — Cierre
- Pregunta: el oráculo y su gold viven en Root, junto al compresor. ¿Queda algo sin tocar?
- Respuesta del usuario: "Cerrar el ticket".

## Pendientes

- Armar el gold necesita trabajo humano: aprobar 80 preguntas de reglas y elegir 10 tareas. La spec del compresor tiene que incluir ese paso HITL → spec del mapa.
