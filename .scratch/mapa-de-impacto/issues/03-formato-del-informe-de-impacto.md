# Formato del informe de impacto
Type: grilling
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿Qué muestra el informe de impacto que Claude pide antes de editar, y dónde vive?

Puntos a decidir con el usuario:

- Orden: lectores y almacenes primero (pilar 2: nada crítico en el medio).
- Tamaño máximo en tokens, y qué se recorta primero cuando el corte no entra.
- Contenido por almacén: las cinco preguntas de contrato (ventana, precisión y unidades, segmentos, cache, escala).
- Lugar: tool MCP de mmorch, extensión de codegraph, o skill.
- Disparo: ¿lo pide Claude a mano, o un hook PreToolUse lo inyecta antes de un Edit?

## Answer

Resuelto en grilling con el usuario el 2026-09-25; captura: `brainstorms/2026-09-25-informe-de-impacto.md`. Alcance: cualquier repo, incluidos ajenos. Disparo en Claude Code: hook `PreToolUse` con `additionalContext`, una vez por archivo por sesión, sin `permissionDecision`, fail-open. Disparo en Cursor: hook `postToolUse` global tras la primera edición de cada archivo, con gate propio. Contenido: (1) lectores por literales compartidos, (2) una línea con los tests que importan el módulo. Tamaño: orden por especificidad; un literal en más de 5 módulos se resume con su conteo; tope de 10 líneas con conteo de lo cortado. Código: `mmorch/impacto.py` testeado, hooks finos, test de guarda que falla si un hook referencia un módulo de mmorch inexistente; registro de cada informe en un jsonl con dueño único. Lenguajes: extractor por lenguaje sobre tree-sitter; cada lenguaje entra solo tras pasar el banco del ticket 09 en ese lenguaje. codegraph (medido en el mismo grilling): MCP quitado de Claude Code y telemetría apagada; llamadores = CLI unida con `grep -w`.
