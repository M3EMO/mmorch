# Cupo y límites de cursor-agent sin interfaz
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo se consume y se mide el plan de Cursor cuando se usa `cursor-agent -p`, y qué límites tiene?

- Qué modelos acepta `--model` y cuáles cuentan contra el plan o contra uso por demanda.
- Si existe una forma de leer el consumo por corrida (salida JSON, API de uso, panel).
- Límites de concurrencia y de tasa del modo sin interfaz.
- Si alguna opción manda el trabajo a la cuenta de Anthropic del usuario (lo que sí gastaría cupo de Claude).

Fuentes: documentación oficial de Cursor y del CLI `cursor-agent`, y `cursor-agent --help`.

## Answer

Resuelto (2026-09-29, research AFK). Detalle y fuentes: [research/02-cupo-cursor.md](../research/02-cupo-cursor.md).

- `cursor-agent` no gasta cupo del plan Claude: usa solo la suscripción de Cursor, también con modelos `claude-*` (staff de Cursor, enero 2026; el `--help` no tiene flag de key propia). Confianza media-alta.
- El plan individual tiene dos pools mensuales: "Cursor Models" (Composer 2.5, Grok) y "Other Models" a precio de API. `auto` cobra al precio del modelo que elige. Para el spike conviene fijar `--model` del pool "Cursor Models".
- `--list-models` lista 246 ids.
- Consumo por corrida: el evento `result` del JSON trae `usage` (tokens de entrada, salida y cache), sin costo en dólares; el modelo sale en `system/init`. El costo está en el panel de Cursor.
- Sin límite de concurrencia documentado.
- Modo desatendido: auth con `CURSOR_API_KEY` o `--api-key`; hace falta `--trust` o `--force` en un workspace nuevo; sin `--force`, `-p` solo propone cambios. En falla sale con código distinto de 0 y el modo `json` no emite JSON válido.
- Sin verificar: montos por pool, un `usage` real, el límite real de concurrencia.
