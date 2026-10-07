# Inventario de aristas invisibles en mmorch
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cuántas aristas que codegraph no ve tiene mmorch, y de qué clase? Contar con ejemplos por clase:

- **Datos:** archivos jsonl y json compartidos, tablas SQLite (chat.db, workflow.db), variables de entorno, claves JSON leídas por string.
- **Indirectas:** funciones pasadas como valor (`signal_fn=collect_signals`), registros por decorador (`@_tool` en mcp_server), despacho por string (`getattr`, dicts de handlers).
- **Ejecución:** scripts lanzados por nombre en un subproceso (`scripts/gates.py`), hooks de Claude Code, jobs nocturnos, entradas de CLI.

Para cada clase: cuántas hay, un ejemplo con archivo y línea, y si `codegraph_trace` la sigue. La descripción de codegraph dice que sigue callbacks; hay que verificarlo con un caso real.

Salida: tabla en `research/02-inventario-aristas.md`. Esa tabla prioriza los tickets 04 y 05.

## Answer

codegraph no ve ninguna de las cuatro clases en Python. Datos: ~89 almacenes y ~75-80 pares escritor→lector; el centinela `loop_paused` tiene 11 lectores; 43 variables de entorno. Indirectas: ~104 funciones pasadas como valor, 67 decoradores de registro, 12 despachos por string. Ejecución: ~26 aristas. codegraph no sigue callbacks: `callers(collect_signals)` y `trace` desde `run_nightly` vuelven vacíos. Tres hallazgos verificados: dos hooks llaman a `mmorch.context_blocks`, borrado en 19601ef; el kill-switch de auto_apply escribe en otra carpeta que sus 11 lectores; el reindex incremental de codegraph borra en cascada los llamadores entrantes del archivo editado (reproducido en 0.9.6 con un repo de 2 archivos). Prioridad: datos → indirectas → ejecución. Detalle: [research/02-inventario-aristas.md](../research/02-inventario-aristas.md). Aceptado por el usuario el 2026-09-24.
