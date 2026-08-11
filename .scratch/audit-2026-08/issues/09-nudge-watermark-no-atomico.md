# Watermark distill_upto (nudge.json): write no atómico + dos escritores sin lock → re-destilado

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: robustez
Evidence: mmorch/nudge.py:20-26,50,57-58 · scripts/nightly.py:97-120

nudge.json corrupto → `_load` cae al default sin `distill_upto` → el próximo destilado
arranca en `after_id=0`. Dos procesos (nudge.tick diurno en el MCP server; nightly.py
02:10) hacen load→modify→write completo sin lock: last-writer-wins puede retroceder el
watermark. Resultado: `distill_backlog` re-procesa episodios ya destilados — hasta
50/noche de llamadas gen+verify repetidas + notas duplicadas en memory.duckdb.

**Fix:** write atómico + preservar watermark ante corrupción; mejor: mover
`distill_upto` a memory.duckdb (transaccionalidad ya disponible).

## Comments
Fix mínimo (NO migré a duckdb, fuera de scope de este ticket): `nudge.json` ahora usa
`iohelpers.atomic_write_json` (elimina la causa raíz de la corrupción — el truncado
mid-write del crash nocturno) + `load_json_tolerant` con log fuerte. Para el
last-writer-wins entre `nudge.tick` (MCP server) y `nightly.py`, agregué un guard
monotónico en `tick()`: relee el archivo justo antes de escribir y nunca deja que
`distill_upto` retroceda en disco. No es un lock inter-proceso completo, pero cierra el
caso concreto del ticket (re-destilado por watermark retrocedido) sin nueva dependencia.
