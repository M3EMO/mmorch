# Oráculo de fidelidad del compresor
Type: grilling
Status: resolved
Blocked by: 01, 03
Map: ../map.md

## Question

Decidí con el usuario cómo se mide que una versión comprimida conserva lo que importa. Las opciones conocidas son una batería de preguntas con respuesta exacta, una batería de tareas reales y la contención de hechos. El oráculo no puede ser un LLM-juez. Fijá el piso de fidelidad y cuántos casos necesita.

## Answer

Resuelto con el usuario el 2026-11-26 en un grilling. Captura completa: `vault/brainstorms/2026-11-26-vault-y-compresor-oraculo-de-fidelidad.md`.

- El oráculo tiene dos piezas, y ninguna usa un LLM-juez (usuario: "1 y 4").
- La primera pieza es una batería de preguntas con respuesta exacta. Las preguntas de ruteo piden un pedido real → nombre del skill. Las preguntas de reglas piden un dato del CLAUDE.md, AGENTS.md o MEMORY.md → respuesta exacta. Un checker compara cada respuesta.
- La segunda pieza es un gate final de tareas reales. Corre una vez antes de aceptar una versión comprimida. Un checker determinista juzga cada tarea: test que pasa, archivo creado o skill invocado en el transcript.
- Haiku 5.5 responde la batería por API, sin cupo, en cada iteración del compresor. El original y el comprimido corren con el mismo modelo.
- El gate de tareas corre con el modelo real en `claude -p`.
- La retención es igual a los aciertos del comprimido divididos por los aciertos del original, sobre los casos que el original acierta.
- Piso: la batería retiene 0.965 o más (cifra de SkillReducer). Las 20 preguntas de guardrails y de zona roja retienen el 100%. El comprimido pasa toda tarea que el original pasa.
- Casos: 200 en la batería (120 de ruteo, unos 25 con respuesta "ningún skill", y 80 de reglas). Cada caso corre 3 veces y vale la mayoría. El gate tiene 10 tareas.
- Gold: el ruteo sale de transcripts reales. Un LLM redacta las preguntas de reglas, y el usuario aprueba cada una. El usuario elige las 10 tareas. Un caso entra solo si el original lo acierta en 2 de 3 corridas.
- El oráculo y su gold viven en Root, junto al compresor.
- La spec tiene que incluir el armado del gold como paso HITL.
