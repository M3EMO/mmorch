# Dónde entra Cursor como trabajador de Claude: Brainstorm / Discovery Notes
Date: 2026-09-30 · Goal: resolver el ticket 03 del mapa `.scratch/cursor-trabajador` (coder de SDLC, nivel de la escalera o receptor de tickets completos).

## Summary / key decisions
- Primer objetivo: recortar cupo de Claude en las sesiones interactivas; Cursor recibe tickets completos. SDLC queda para una segunda etapa.
- Alcance: cualquier tarea de implementación (no solo las que tienen test previo); Claude verifica leyendo el diff, y corre los tests que existan.
- Umbral para delegar: el pedido entra en 10 líneas o menos y el cambio toca varios archivos o supera ~30 líneas. Claude hace él mismo los cambios de 1-2 líneas y los que piden decisiones de diseño al escribir.
- Repos: ninguno excluido, Portfolio incluido. En cambio, algunas acciones necesitan permiso: Cursor frena y pregunta; Claude decide y lo reanuda.
- Acciones con permiso: scripts de brokers u órdenes; instalar paquetes o descargar; borrar archivos; tocar credenciales, `.env` o configuración de agentes; commit, push o merge. Libre: leer, editar código y correr tests. Si toca dinero, Claude pregunta al usuario.
- Rechazo: una sola vuelta. Claude reanuda la sesión de Cursor con comentarios concretos; si vuelve a fallar, Claude termina la tarea o descarta el worktree.
- Modelos de Cursor: solo baratos; el banco compara `composer-2.5`, `grok-4.7-medium`, `gemini-3.8-flash-medium` y `gpt-5.4-mini-medium`.
- Regla de ruteo: por la forma de la tarea. API DeepSeek para tareas de una llamada (generar, clasificar, verificar, un archivo con contexto armado). Cursor para tareas de agente (explorar el repo, varios archivos, correr tests e iterar). El coder de SDLC es zona gris y lo decide el banco.
- Roles de Cursor: explorador del repo (resumen corto para Claude), armador de borradores de plan o spec (Claude aprueba), implementador. No es gerente intermedio: no reparte trabajo a otros agentes; la jerarquía queda plana.
- Herramientas de mmorch en Cursor: solo `mmorch_check` (oráculo) para verificar su propio trabajo; nada que delegue (`mmorch_route` y similares).

## Contexto de partida
- Spike (ticket 01): `cursor-agent` con `composer-2.5` corre desatendido, ~95 s por tarea, acierta 2/3; no gasta cupo de Claude (ticket 02).
- Jerarquía fija: Claude superior, delegación solo Claude → Cursor, Cursor en worktree con `--force`, sin push ni merge.

## Q&A log
### Q1 — Qué gasto de cupo de Claude recortar primero
- Asked: ¿sesiones interactivas (Claude escribe código a mano) o SDLC nocturno (Claude revisa/arregla)?
- Dato del repo: de 35 corridas de SDLC, 32 llamaron a Claude (48 llamadas en total).
- Captured: el usuario confirmó empezar por las sesiones interactivas, con Cursor como receptor de tickets completos: Claude escribe el spec y verifica, Cursor implementa. SDLC queda para una segunda etapa.
- Flags: ninguno.

### Q2 — Qué trabajo puede recibir Cursor
- Asked: A = solo tareas con test de aceptación escrito antes por Claude (verifica con el test); B = cualquier tarea de implementación (verifica leyendo el diff). Recomendé A.
- Captured: el usuario eligió **B**. Cursor puede recibir cualquier tarea de implementación; Claude verifica leyendo el diff (y corre los tests que existan, donde haya oráculo).
- Flags: leer el diff también gasta cupo de Claude; el ahorro neto se mide en el banco (ticket 04 del mapa). En lo checkeable sigue valiendo el invariante: oráculo antes que juicio.

### Q3 — Cuándo delega Claude
- Asked: umbral por costo fijo (~95 s de Cursor + lectura del diff). Recomendé: pedido de 10 líneas o menos, y cambio de varios archivos o más de ~30 líneas.
- Captured: el usuario confirmó los umbrales. Claude no delega cambios de 1-2 líneas ni los que necesitan decisiones de diseño durante la escritura.
- Flags: los umbrales son una estimación; el banco puede corregirlos.

### Q4 — Repos excluidos
- Asked: recomendé excluir Portfolio mientras Cursor corra con `--force` (scripts de brokers y órdenes con dinero real).
- Captured: el usuario dijo "No exclusion pero me gustaria que te tenga que pedir permiso para algunas cosas". Ningún repo queda excluido; ciertas acciones requieren permiso de Claude.
- Mecanismo disponible (verificado en docs de Cursor): `<worktree>/.cursor/cli.json` con `permissions.allow`/`deny` (`Shell(cmd)`, `Read(glob)`, `Write(glob)`, `WebFetch(dominio)`, `Mcp(server:tool)`); deny gana sobre allow. Segunda capa: Cursor también corre los hooks `PreToolUse` de `~/.claude/settings.json` (hallazgo del spike), que mmorch controla. Flujo de permiso: Cursor choca con un deny, termina su turno diciendo qué necesita, Claude decide y lo reanuda con `--resume <chatId>`.
- Flags: no está documentado cómo interactúa `--force` con las reglas deny en modo `-p` -> verificarlo antes del banco.

### Q5 — Qué acciones necesitan permiso
- Asked: lista recomendada de cinco acciones con permiso; el resto libre.
- Captured: el usuario confirmó la lista: (1) scripts que llaman a brokers o mandan órdenes, (2) instalar paquetes o descargar de internet, (3) borrar archivos, (4) tocar credenciales, `.env` o configuración de agentes, (5) commit, push o merge. Libre: leer, editar código y correr tests. Si la acción toca dinero, Claude le pregunta al usuario antes de dar permiso.
- Flags: traducir la lista a reglas `deny` de `cli.json` y a un hook `PreToolUse` (ticket del protocolo).

### Q6 — Qué pasa si Claude rechaza el diff
- Asked: recomendé una vuelta de corrección con comentarios concretos (`--resume`), y después Claude termina o descarta.
- Captured: el usuario confirmó el tope de una vuelta.
- Flags: ninguno.

### Q7 — Qué modelo usa Cursor
- Asked: recomendé `composer-2.5` por defecto y `grok-4.7-medium` como segundo brazo, ambos del pool "Cursor Models".
- Captured: el usuario respondió "Modelos grok y usar otros modelos pero los 'baratos'". Entran los Grok y otros modelos baratos; nada caro.
- Disponibles baratos (de `--list-models`, 2026-09-30): pool "Cursor Models": `composer-2.5`, `grok-4.7-low|medium|high`, `cursor-grok-4.6-*`; fuera del pool, a precio de API pero baratos: `gemini-3.8-flash-*`, `gemini-3.7-flash-*`, `gpt-5.4-mini-*`.
- Confirmado: el banco tiene cuatro brazos de Cursor: `composer-2.5`, `grok-4.7-medium`, `gemini-3.8-flash-medium` y `gpt-5.4-mini-medium`. El banco elige el modelo por defecto por acierto y costo; si gana uno de afuera del pool, se decide con el usuario si vale su precio.
- Flags: ninguno.

### Hilo nuevo abierto por el usuario (2026-09-30)
- "como definimos como ir por api deepseek propia o ir via cursor? cuales son las funciones de cursor? como un gerente intermedio? un planner? armador de specs? exploremos"
- Dos preguntas: (a) regla de ruteo entre la API DeepSeek de mmorch y Cursor; (b) qué roles cumple Cursor además de implementar.

### Q8 — Regla de ruteo entre API DeepSeek y Cursor
- Asked: decidir por la forma de la tarea, no por el modelo.
- Captured: el usuario confirmó. API DeepSeek de mmorch = tareas de una llamada (generar, clasificar, verificar, escribir un archivo con el contexto ya armado): segundos, paralelo, dólares bajos, medido. Cursor = tareas de agente (explorar el repo, tocar varios archivos, correr tests e iterar): ~95 s, cupo del plan de Cursor, sin medir salvo el spike. Zona gris: el coder de SDLC (hoy DeepSeek), lo decide el banco del ticket 04.
- Flags: ninguno.

### Q9 — Roles de Cursor además de implementar
- Asked: explorador, armador de plan/spec, implementador, gerente intermedio. Límite: el juicio del orquestador nunca se delega; los borradores sí, con aprobación de Claude.
- Captured: el usuario confirmó. Sí: explorador del repo (lee mucho, devuelve resumen corto, ahorra contexto y cupo de Claude); armador de borradores de plan o spec (Claude aprueba o corrige); implementador. No: gerente intermedio (Cursor no reparte trabajo a otros agentes; cada nivel extra complica verificar y depurar, y planificar es juicio de orquestador).
- Flags: el banco del ticket 04 mide implementación; explorar y redactar borradores todavía no tienen banco.

### Q10 — Cobertura: herramientas de mmorch dentro de Cursor
- Asked: Cursor ya tiene el MCP de mmorch configurado. Recomendé permitir solo `mmorch_check` (oráculo) y prohibir las que delegan (`mmorch_route` y similares).
- Captured: el usuario confirmó. Cursor verifica su trabajo con el oráculo, pero no delega a la API DeepSeek; la jerarquía queda plana.
- Flags: expresar la restricción con `Mcp(mmorch:...)` en `cli.json` (ticket del protocolo).

## Open flags (pending input)
- Roles de explorador y de borrador de plan/spec sin banco que los mida -> nuevo ticket del mapa.
- Interacción de `--force` con `permissions.deny` en `cursor-agent -p` -> verificar con una corrida mínima.
- Costo de verificar leyendo el diff contra el ahorro de no escribir el código -> lo mide el banco comparativo (ticket 04).
