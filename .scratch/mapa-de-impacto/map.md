# Mapa wayfinder — mapa de impacto para entender un proyecto entero
Label: wayfinder:map · Creado: 2026-09-24 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Antes de editar un archivo en un repo grande, propio o ajeno, el agente recibe un **informe de impacto** automático: los otros módulos que comparten literales de datos con ese archivo. El informe llega por hook (Claude Code antes de editar, Cursor después de la primera edición). Se mide con un oráculo: tareas de cambio con un lector acoplado y tests ocultos. Cada lenguaje entra solo si pasa ese banco.

(Destino original, reemplazado el 2026-09-25: un corte con cuatro clases de aristas medido por recall contra codegraph. El ticket 01 mostró que esa métrica satura en 1.00 y no ve los escapes.)

## Notes

- Origen: video "AI gives too much code" (youtube k2qls2LiBRc), tres pilares. Pilar 1: la ventana esconde aristas y cuadráticos. Pilar 2: la atención dispersa pierde el medio. Pilar 3: el retrieval no ve el acople sin referencia en el código.
- La meta no es "entender todo". La meta es no perder ningún lugar afectado por un cambio. Esa meta es medible; "entender todo" no.
- Evidencia de 2026-09-24 (leer antes de resolver tickets):
  - `codegraph_impact(log_event)` devolvió 54 escritores y 0 lectores de metrics.jsonl.
  - Banco de acople: vault `research/banco-de-acople-por-datos-*`; código en el worktree `../orch-bench`, rama `bench/data-coupling`, carpeta `scripts/bench_acople/`.
  - Registro tipado + dueño único + banco de escala: gate de 26/40 a 33/40 por 120 líneas (commit 2f14d4f en mmorch/auto).
  - Contratos generales sobre held-out: cierre real 25% (puntuales) → 71% y 57% (contratos). La ronda 4 con juez de equivalencia estaba corriendo al crear este mapa.
  - Punto ciego nuevo: `signal_fn=collect_signals` (inyección por parámetro). Un grep de `collect_signals(` no lo ve, y el juez v1 marcó 4 mutantes vivos como camino muerto.
- Datos reutilizables: más de 100 mutantes etiquetados con tests que fallan (`results.jsonl`, `heldout_r*_results.jsonl`). Ojo: el gate corrió con `-x`, así que solo guarda el PRIMER test que falla.
- Invariantes: checkeable → oráculo, nunca un LLM; medir primero; escalera ponytail (stdlib y codegraph antes que código nuevo); resolver tickets es HITL; research al vault (`mmorch_vault_write`, tag `mmorch`).
- Escalera de extracción de aristas, de barato a caro: strings estáticos (rutas, tablas, claves) → `sys.addaudithook` sobre la suite (stdlib: `open`, `sqlite3.connect`, `subprocess.Popen`) → análisis AST de funciones como valor.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Banco de tareas con lector acoplado](issues/09-banco-de-tareas-con-lector-acoplado.md) — POSITIVO: el informe estático de literales compartidos sube el acierto en tareas acopladas de 42% a 96% (deepseek-v4-pro, p=4e-5) sin costo en control; sin informe, toda falla es el error del video. Justifica los tickets 03 y 05.
- [Spike de recall de impacto](issues/01-spike-aristas-de-datos-y-recall.md) — negativo: A y B dan recall 1.00, porque todo test que falla importa el módulo mutado; la métrica no ve escapes. codegraph 0.9.6 no resuelve alias de módulo; 1.6.0 sí, y además conserva llamadores al editar.
- [Inventario de aristas invisibles](issues/02-inventario-de-aristas-invisibles.md) — codegraph no ve datos (~89 almacenes), indirectas (~104 funciones como valor, sin seguir callbacks) ni ejecución (~26); dos bugs reales (hooks a módulo borrado, kill-switch en otra carpeta) y el reindex incremental borra llamadores entrantes. Prioridad: datos → indirectas → ejecución.
- [Formato del informe de impacto](issues/03-formato-del-informe-de-impacto.md) — grilling: cualquier repo; hook `PreToolUse` en Claude Code y `postToolUse` en Cursor (con gate); lectores por literales + línea de tests; orden por especificidad, tope 10 líneas; `mmorch/impacto.py` + test de guarda de hooks; tree-sitter por lenguaje, cada uno tras pasar el banco. codegraph medido: MCP quitado, llamadores = CLI + `grep -w`. Salen los tickets 05 (implementar), 10, 11 y 12.
- [Implementar el informe](issues/05-herramienta-de-impacto.md) — HECHO: `mmorch/impacto.py` + hooks en Claude Code y Cursor + test de guarda; aceptación 23/24 acopladas. Una línea de tests se quitó: no mejoró (20/24).
- [Repo real: Portfolio](issues/07-repo-ajeno-sin-registro.md) — APROBADO: en código real del usuario, acopladas 54% → 96% (+42, p=0.0009), control 100% → 92%. El co-cambio en git resultó una verdad débil; el banco de tareas sí mide el caso del video.
- [Extractor TypeScript](issues/11-extractor-typescript.md) — APROBADO: acopladas 38% → 100% en TS (p=1e-6); v1 (solo literales) instalada con tree-sitter opcional. Alcanza con señalar al módulo que comparte datos.
- [Extractor Java](issues/12-extractor-java.md) — APROBADO: acopladas 38% → 96% en Java (p=1.4e-5), control 100% → 100%; `.java` instalado con tree-sitter opcional. Límite: un escritor sin literales (clase de acceso a datos) da informe vacío.
- [Aristas indirectas](issues/04-aristas-indirectas-estaticas.md) — APROBADO: con literales solo, acopladas 38%; con aristas indirectas, 92% (p=9e-5), control 100%. Sin ayuda fallan kwarg, `getattr` con prefijo y atributo asignado; lista, dict y `key=` las resuelve grep. Instalado en Python con cache de hechos.
- [Repos sin tests](issues/14-repos-sin-tests.md) — cerrado sin cambios: sin tests ni `run_tests`, acopladas 21/24 contra 22/24 con tests; el agente llega al lector por el informe, no por los tests.
- [Lectores fuera del repo](issues/13-lectores-fuera-del-repo.md) — APROBADO P, R fuera: con lector en un prompt o hook, 0/12 → 12/12 (p=4e-7); 18 de 20 líneas reales apuntan a lectores reales. Repos hermanos: +25 no significativo y puro ruido en repos reales. De paso, el hook dejó de importar openai y volvió a entrar en su corte.
- [Costo propagado](issues/15-costo-propagado.md) — APROBADO con cambio de método: anotar al inicio no sirve (la función trampa vive en un módulo que el archivo aún no importa); el aviso después de escribir sube trampas de 33% a 100% (p=3e-7). Instalado como hook `PostToolUse` con cache; 7 de 8 avisos reales en el repo.
- [Scaffold con registro](issues/16-scaffold-con-registro.md) — cerrado sin cambios: con un dueño de API estable, el agente lo usa aunque haya un lector directo (S0 23/24, S1 24/24); el registro tipado y el test de contrato no suman. `/new-project` queda igual.
- [Gate de Cursor](issues/10-gate-de-cursor.md) — APROBADO: el informe después de la primera edición sube acopladas de 42% a 79% (+37, p=0.009) sin costo en control; rinde menos que al inicio (96%). Cursor entra con `postToolUse`.
- [Transferencia de contratos](issues/06-transferencia-a-otra-superficie.md) — cerrado sin hacer: los contratos no convergen; el valor vino del informe y de cambios estructurales.
- [Empaquetado](issues/08-empaquetado.md) — resuelto por el 03: hooks finos sobre un módulo de mmorch; sin tool MCP ni skill.

## Not yet specified

Los cuatro tickets abiertos el 2026-09-27 desde esta sección (13-16) quedaron resueltos el 2026-09-29.

## Out of scope

- Resúmenes LLM del repo como fuente de impacto: pierden información; sirven solo para navegar.
- Agrandar la ventana de contexto como solución (pilar 1: costo O(n²) y "lost in the middle").
- `codegraph explore` para navegar: medido sin valor (+22% tokens, mismo acierto) el 2026-09-25.
