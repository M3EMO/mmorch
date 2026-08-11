# memory._connect re-ejecuta DDL + migración en cada operación de memoria

Type: task
Status: open
Severity: NICE-TO-HAVE
Effort: S
Eje: eficiencia
Evidence: mmorch/memory.py:83-127,194,327,395,456

Cada op abre conexión DuckDB + 2 CREATE TABLE + 2 CREATE SEQUENCE + scan de
information_schema + loop de 6 columnas; recall_hybrid abre 3 conexiones/llamada en un
proceso MCP residente.

**Fix:** flag módulo-level "schema migrado" (DDL una vez por proceso); NO compartir la
conexión entre threads sin lock.

## Comments
`_MIGRATED_PATHS: set` (heredado) saltea el DDL/ALTER TABLE una vez por path de DB por
proceso; cada `_connect()` sigue abriendo una conexión DuckDB nueva (no se comparte entre
threads). Verificado con `tests/test_memory.py`. Cierro.
