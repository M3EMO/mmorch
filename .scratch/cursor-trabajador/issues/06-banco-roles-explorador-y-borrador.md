# Banco de roles: Cursor como explorador del repo y como borrador de plan
Type: prototype
Status: resolved
Blocked by: 04
Map: ../map.md

## Question

¿Cursor como explorador del repo (devuelve un resumen corto) y como borrador de plan o spec (Claude aprueba) ahorra cupo de Claude sin bajar la calidad de lo que Claude decide después? El banco del ticket 04 solo mide implementación. Diseño y criterio se pre-registran antes de correr; el oráculo puede reusar las preguntas de navegación de `orch-spike/scripts/nav`.

## Notes

- Decidido en el protocolo (2026-10-01): el rol de explorador compara la búsqueda semántica de Cursor (`semSearch`) contra el grep de Claude.
- Medido (2026-10-01, CLI 2026.09.28 y 2026.10.01): `semSearch` no existe en `cursor-agent -p`. Faltó en una carpeta suelta, en un repo git y en el repo de mmorch con remoto de GitHub. El explorador de Cursor usa grep, glob y read, igual que Claude. La comparación "semSearch contra grep" no se puede hacer sin interfaz.
- Medido (2026-10-01): en modo normal con `--force`, Cursor ve las herramientas MCP de sus plugins de Gmail, Calendar y Drive, incluidas `trash_message` y `delete_event`. El servidor `mmorch` no carga sin interfaz. `deny: ["Mcp(*:*)"]` en `<workspace>/.cursor/cli.json` bloquea la llamada (verificado). `--mode ask` y `--mode plan` quitan las herramientas MCP.
- Pre-registro (2026-10-01): [research/06-preregistro.md](../research/06-preregistro.md). Código: `orch-spike/scripts/cursor_roles/` (`roles.py`, `analyze.py`). Decisiones del usuario: Opus, gasto escalonado con anclaje, rol B piloto con implementación, umbrales sin cambios, y comparación de cuatro borradores sin Opus (Cursor, DeepSeek, Haiku, Sonnet).
- Congelado (2026-10-01) en el commit local `fcb7ea4` de orch-spike, antes de la primera corrida de Opus. SHA-256: `roles.py` 84bbc706…87d5e11, `analyze.py` 8a782d28…1392dc4, `items_frozen.json` f0098c5b…d5879421, `planted.json` a5c5380f…e64a9bf6c.
- Fase 0 (sin Opus): la compuerta pasa en A (0.978, parseo 1.0) y en B (0.861, parseo 1.0). Plantado: A con 17 plantados y 3 naturales; B con 11 plantados y 1 natural. Calibración del implementador: `c01` pasa 1 de 3 con el plan oráculo y queda como ruido.
- Desvío posterior al congelamiento: `analyze.py` fallaba al serializar un `numpy.bool_`. El commit `7af2827` agrega un `default` a `json.dumps`; la regla no cambia. SHA-256 nuevo: 241a352e…3203bd1c.
- Mirada intermedia del rol A (2026-10-01, 25 ítems, `veredicto_interim.json`): CORTAR POR FUTILIDAD. Calidad A0 0.952 y A1 0.972 (d +0.021, cota inferior 0.0). Cociente de cupo neto 0.874 (ahorro 12.6%, menor que el corte de 15%), cota superior 0.986. Anclaje: 0 de 8 plantados adoptados, caída 0. Cupo de sesión extrapolado: 0.82 con 50k y 0.81 con 150k tokens de contexto. Gasto: 66 corridas de Opus, US$4.12 equivalentes. El veredicto pre-registrado del rol A queda en ese corte.
- Decisión del usuario (2026-10-01): correr igual los 25 ítems restantes del rol A, para una comparación completa. El análisis de 50 ítems usa la misma regla congelada y se informa como continuación pedida, aparte del veredicto pre-registrado. El usuario también aprobó el rol B con Opus.
- Brazo exploratorio posterior al congelamiento (pedido del usuario, 2026-10-01): `deepseek-reasoner` como planner, con el mismo agente con herramientas que el brazo `deepseek-v4-pro`. Pregunta: ¿qué modelo conviene en lugar del reasoner como `WRITER` de SDLC? Limitación: en SDLC el reasoner planea con una sola llamada desde la spec, sin explorar el repo.

## Answer

Resuelto (2026-10-01) con el banco pre-registrado ([research/06-preregistro.md](../research/06-preregistro.md)). Código y resultados: `orch-spike/scripts/cursor_roles/` (congelado en `fcb7ea4`). Claude: Opus 5.5 (`claude` 2.1.287). Cursor: `grok-4.7-medium` (2026.10.01).

- **Explorador (rol A, 50 preguntas sobre mmorch): NO ADOPTAR.** La calidad no baja: Opus solo acierta 0.967 y con el resumen de Cursor 0.979 (diferencia +0.011, cota inferior +0.001). El ahorro de cupo es 14.7% (cociente 0.853, cota superior 0.921), lejos del 30% pedido. El veredicto pre-registrado es el corte por futilidad a los 25 ítems (ahorro 12.6%); los 25 restantes corrieron por pedido del usuario y confirman el resultado.
- **Borrador de plan (rol B, piloto de 12 tareas): NO ADOPTAR.** Opus planea bien las 12 tareas con borrador y sin él. Con el borrador de Cursor gasta 5.7% más (cociente 1.057). Con el implementador, los planes de Opus pasan los tests ocultos en 0.97-1.00.
- **Anclaje: ninguno.** Opus no adoptó ninguno de los 28 defectos plantados (17 en A, 11 en B). Opus marcó como "corregido" los 12 borradores plantados de B.
- **Por qué no ahorra.** Opus resuelve una búsqueda con 4 o 5 llamadas baratas (US$0.06-0.08) y verifica el borrador antes de usarlo. Un resumen ahorra poco cuando explorar ya cuesta poco.
- **Planners sin Opus (pedido del usuario, 12 tareas × 3).** Tareas acopladas con todos los lectores en el plan: Sonnet 24/24, Cursor 19/24, Haiku 18/24, DeepSeek v4 pro 17/24, `deepseek-reasoner` 15/24 (brazo exploratorio). Fisher unilateral contra Sonnet, por corrida: p = 0.025 (Cursor), 0.005 (v4 pro), 0.0008 (reasoner). Con el implementador: Sonnet 1.000, Cursor 0.861, Haiku y v4 pro 0.778, reasoner 0.750. Recurso por plan: Sonnet US$0.034 equivalentes de cupo (41% de un plan de Opus); Haiku US$0.017; Cursor ni dólares ni cupo; v4 pro US$0.006 y reasoner US$0.0007 de API.
- **Sin interfaz.** `semSearch` no existe en `cursor-agent -p`. Cursor ve Gmail, Calendar y Drive por sus plugins; `deny: ["Mcp(*:*)"]` los bloquea.
- **Gasto del banco.** Opus: 161 corridas (más 4 de humo), US$11.34 equivalentes. Haiku y Sonnet: 72 corridas, US$1.83 equivalentes. Cursor: 186 corridas. DeepSeek: US$4.08 de API.
- **Desvíos.** Un arreglo de serialización en `analyze.py` (`7af2827`) y el brazo exploratorio del reasoner, posterior al congelamiento. La primera tanda del reasoner falló por el arnés: el modelo puso `summary` fuera de `args`. Las filas viejas quedan en `*_reasoner_v0.jsonl`.
- **Consecuencia para el mapa.** Cursor rinde como implementador (ticket 04), no como explorador ni como borrador para Opus. Para SDLC, que no tiene a Opus en la etapa de plan, el mejor planner medido es Sonnet; Cursor es la opción gratis.
