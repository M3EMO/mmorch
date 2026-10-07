# Dónde entra Cursor: coder de SDLC, nivel de la escalera o tickets completos
Type: grilling
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿En qué punto del trabajo conviene delegar en Cursor? Candidatos: coder de las etapas 4 y 5 de SDLC (reemplaza a DeepSeek), nivel de la escalera antes de Claude, o receptor de un ticket de bd completo con spec escrito por Claude. Se decide con el resultado del spike.

## Answer

Resuelto por grilling con el usuario (2026-09-30). Captura completa: [brainstorms/2026-09-30-donde-entra-cursor.md](../../../brainstorms/2026-09-30-donde-entra-cursor.md).

- Primero las sesiones interactivas: Cursor recibe tickets completos; Claude escribe el pedido y verifica. SDLC queda para una segunda etapa (hoy 32 de 35 corridas de SDLC llaman a Claude).
- Cualquier tarea de implementación; Claude verifica leyendo el diff y corriendo los tests que existan.
- Delega si el pedido entra en 10 líneas o menos y el cambio toca varios archivos o supera ~30 líneas.
- Ningún repo excluido; cinco acciones piden permiso (brokers u órdenes, instalar o descargar, borrar, credenciales o config de agentes, commit/push/merge). Si toca dinero, Claude pregunta al usuario.
- Una sola vuelta de corrección; después Claude termina o descarta.
- Solo modelos baratos; el banco compara `composer-2.5`, `grok-4.7-medium`, `gemini-3.8-flash-medium`, `gpt-5.4-mini-medium`.
- Ruteo por forma de la tarea: una llamada → API DeepSeek; agente (explorar, varios archivos, tests) → Cursor; el coder de SDLC lo decide el banco.
- Roles: explorador del repo, borrador de plan o spec (Claude aprueba), implementador. No gerente intermedio. Dentro de Cursor, solo `mmorch_check` de las herramientas de mmorch.
