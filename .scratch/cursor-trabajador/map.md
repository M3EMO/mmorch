# Mapa wayfinder — Cursor como trabajador de Claude
Label: wayfinder:map · Creado: 2026-09-29 · Cerrado: 2026-10-02 (destino alcanzado) · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Cierre

- Los seis tickets quedaron resueltos. La decisión respaldada por bancos es esta: Cursor rinde como implementador (ticket 04), con el protocolo del ticket 05; no rinde como explorador ni como borrador de plan para Opus (ticket 06).
- Lo que sobrevive al mapa pasó a bd: `orchestration-ayz` (despachador en producción, con las tres zonas sin especificar) y `orchestration-34k` (Sonnet o Cursor como planner de SDLC).
- Nota del vault: `vault/research/cursor-como-explorador-o-borrador-para-opus-no-ahorra-cupo-s.md`.

## Destination

Una decisión respaldada por un banco: en qué tipo de trabajo `cursor-agent` sin interfaz rinde como trabajador de Claude, cuánto cupo de Claude ahorra neto de la verificación, y el protocolo para asignar, despertar, devolver y verificar. Implementar el protocolo en producción es el paso siguiente, fuera de este mapa.

## Notes

- Origen: comparación de delegación con Paperclip (2026-09-29). Paperclip despierta agentes al asignarles trabajo; mmorch no. El canal Claude↔Cursor (`mmorch/canal.py`) dice "no despierta al otro" y tiene 2 mensajes en total.
- Jerarquía fija: Claude es el superior. Escribe el spec, asigna, verifica e integra. El juicio del orquestador nunca se delega (CLAUDE.md global, GOAL.md).
- Sentido único: Claude → Cursor. Cursor devuelve un resultado o una pregunta en el canal; no despierta a Claude.
- Permisos del trabajador: `cursor-agent -p --force` siempre dentro de un worktree descartable; nunca push ni merge; Claude revisa antes de integrar.
- Motivo de fondo: Cursor gasta su propio plan; delegarle implementación libera cupo de Claude, el recurso escaso de mmorch.
- Herramienta: `cursor-agent` 2026.08.25 en `C:\Users\map12\AppData\Local\cursor-agent\cursor-agent.cmd`, con `-p`, `--output-format json|stream-json`, `--workspace`, `--model`, `--force`. La costura ya existe: `reviewer_cmd` de `sdlc.toml` y `mmorch.claude_exec.CmdExecutor`.
- El CLI se actualiza solo (2026.08.25 en el spike, 2026.09.28 y 2026.10.01 el 2026-10-01): registrar la versión en cada corrida.
- Seguridad (medido 2026-10-01): sin interfaz y con `--force`, Cursor puede llamar a Gmail, Calendar y Drive por sus plugins. El despachador escribe `deny: ["Mcp(*:*)"]` en `<worktree>/.cursor/cli.json`; esa regla bloquea la llamada (verificado). Detalle en el ticket 06.
- Medición: siempre con oráculo (tests ocultos de los bancos de `orch-spike/scripts/tareas*`). Solo bancos sintéticos: el código de Portfolio no sale hacia Cursor.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Banco de roles: Cursor como explorador y como borrador de plan](issues/06-banco-roles-explorador-y-borrador.md) — no adoptar ninguno de los dos roles: Opus no pierde calidad con el resumen o el borrador, pero ahorra 14.7% (explorador) o gasta 5.7% más (borrador), y no se ancla en ningún defecto plantado (0 de 28). Como planner sin Opus, Sonnet planea las 24 tareas acopladas; Cursor 19, Haiku 18, DeepSeek v4 pro 17, reasoner 15.

- [Protocolo: asignar, despertar, devolver y verificar](issues/05-protocolo-asignar-despertar-devolver.md) — llamada directa en segundo plano con worktree y `git apply`; permisos de terminal por `cli.json` y de archivos por hooks; una vuelta de corrección; reintento y cambio de modelo ante fallas; registro en `logs/cursor_runs.jsonl`.
- [Banco comparativo: Cursor contra el coder de DeepSeek y contra Claude](issues/04-banco-cursor-vs-coders.md) — `grok-4.7-medium` 24/24 (mejor que Claude, 23/24) y queda por defecto; Gemini flash igual de bueno pero 2.5 veces más lento; Composer 2.5 no sirve (14/24); DeepSeek 23/24 sigue como coder de SDLC. Verificar un diff cuesta dos órdenes de magnitud menos que implementar.
- [Dónde entra Cursor: coder de SDLC, nivel de la escalera o tickets completos](issues/03-donde-entra-cursor.md) — primero sesiones interactivas; Cursor recibe tickets completos, explora el repo y redacta borradores; nunca gerente intermedio; cinco acciones piden permiso; solo modelos baratos; ruteo por forma de la tarea (una llamada → DeepSeek, agente → Cursor).
- [Spike: cursor-agent sin interfaz en un worktree sobre 3 tareas del banco](issues/01-spike-cursor-sin-interfaz.md) — corre desatendido (~95 s por tarea, composer-2.5) y acierta 2/3; falla en t01 como el resto de los agentes. Hallazgo: carga los hooks de Claude Code y, lanzado con `SHELL` de Git Bash, los rompe y edita por terminal; el despachador lo lanza sin `SHELL`.
- [Cupo y límites de cursor-agent sin interfaz](issues/02-cupo-y-limites-de-cursor.md) — no gasta cupo de Claude; dos pools ("Cursor Models" y "Other Models" a precio de API); el JSON trae tokens pero no dólares; sin límite de concurrencia documentado.

## Not yet specified

- Promovido a bd `orchestration-ayz` al cerrar el mapa (2026-10-02): concurrencia de varios trabajadores Cursor, secretos dentro del worktree y contabilidad del plan de Cursor en `logs/metrics.jsonl`.

## Out of scope

- Cursor que despierta a Claude (cooperación en los dos sentidos): descartado al fijar el destino (2026-09-29).
- Implementar el despachador en producción: es el paso siguiente al destino; vive en bd `orchestration-ayz`.
- Adoptar Paperclip: evaluado y descartado (`orchestration-r5z`, `vault/research/paperclip-grafts.md`).
