# Gates del SDLC a mmorch: Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir modulo destino, regla de imports, orden de traslado y medicion de precision por gate (bd `orchestration-bpn`).

## Summary / key decisions
- D1: los chequeos viven en `mmorch/gates.py`, funciones puras que devuelven `(ok, detalle)`.
- D2: `tests/test_capas.py` suma R6 (gates y checkers nunca importan `sdlc`, transitivo) y R7 (ratchet de los 9 importadores de `sdlc`).
- D3: orden en tres fases, un gate por commit; `sin-clones` primero; el bd cierra con la fase 1 (9 gates de texto puro).
- D4: en fase 3 la ejecucion queda en `sdlc/`; la decision pura sobre la salida pasa a `mmorch/gates.py`.
- D5: casos etiquetados = incidentes de los tests + mutacion de artefactos (etiqueta por construccion); run-logs con veredicto humano se suman despues. Metrica: accuracy balanceada y falso rechazo.
- D6: la medicion se implementa en este bd (`orchestration-bpn`).
- D7: cada gate de fase 1 se traslada y se mide en el mismo commit; los 9 llevan numero.
- D9: un gate que mide bajo se traslada igual; el README publica el numero y un numero bajo abre un bd. El traslado no cambia comportamiento.
- D10: cada incidente reconstruido lleva una marca en la lista literal de `gates_bench.py`.
- D11: OK al traslado de `sin-clones`. Un caso rojo es una copia con hasta 30% de lineas editadas.
- D12: un caso puede marcarse `aparte`: no entra a la metrica principal y su acierto se publica por grupo (variantes de formato de spec-review).
- D8: `mmorch/gates_bench.py` con semilla; filas en `logs/ablation_results.jsonl`; tabla por gate en README "Measured".

## Contexto medido (antes de la primera pregunta)

### Inventario de gates de `sdlc/pipeline.py` (2026-10-23)

El inventario viejo (`.scratch/sdlc-6-gates/research/01-inventario-gates.md`) describe el engine `/project` de mmorch.
Ese engine no es `sdlc/pipeline.py`. Este inventario es nuevo.
Hay 30 ids de gate en `rec_gate(...)`. Ninguno tiene un numero medido de precision.

Columna "chequeo": la parte que decide verde o rojo. Columna "orquestacion": lo que el pipeline hace alrededor.

| Gate (id) | Funcion | Entrada del chequeo | Chequeo puro | Orquestacion / efectos | Usa mmorch hoy |
|---|---|---|---|---|---|
| sin-clones | `gate_clones` | `written: dict` | si, texto | ninguno | no |
| cambio-minimo | `gate_cambio_minimo` | `_orig_files`, `written`, `TASK.task` | si, con 2 globales | ninguno | no |
| docstring-intacto | `gate_docstring` + `_docstrings` | `before`, archivos en `WT`, `TASK.task` | casi: lee disco | ninguno | no |
| spec-review | `gate_clarificaciones` | `spec_md` | si, texto | ninguno | no |
| trazabilidad-spec | `gate_traza_spec` | `spec_md`, `tests`, `TASK.accept_files` | si, con 1 global | ninguno | no |
| trazabilidad-plan | `gate_traza_plan` + `_filas_tabla` | `spec_md`, `plan_block`, `files` | si, texto | ninguno | no |
| plan-allowlist | `gate_plan_allowlist` | `plan_md`, `files`, `_need_files()`, techo, `TESTS_PREFIX` | si, con config | ninguno | no |
| tamano-plan | `gate_tamano` + `_tamano` | `files`, tamaños en `WT`, umbrales | no: lee disco | escribe `state["plan_tamano"]` | `mmorch.impacto.lectores` |
| baseline-intacto | `gate_baseline` | `SNAP`, hashes en `WT` | no: lee disco | ninguno | no |
| alcance (gid variable) | `gate_alcance` | `before`, `_tree()` | no: git | revierte archivos fuera de alcance | no |
| G3-compile | `gate_compile` + `_compila` | archivos, `compile_cmd` | no: ejecuta | baseline de errores en `_COMPILE_BASE` | `mmorch.sandbox` via `sh` |
| test-compile | `gate_test_compile` | tests de aceptacion | no: ejecuta | ninguno | idem |
| codigo-muerto | `gate_codigo_muerto` + `_cobertura` | `.py` nuevos, aceptacion | no: ejecuta | `state["cobertura_nueva"]`, umbral `cobertura_min` | `mmorch.sandbox` |
| mutacion | `gate_mutacion` | archivos del plan, lineas cambiadas | no: escribe mutantes y ejecuta | `state["mutation_score"]`, umbral `mutation_min` | `mmorch.checkers._mutants` |
| suite-total | `gate_suite_total`, `_suite_cmd_total` | suite del repo, baseline por sha | no: ejecuta | cache de baseline, reintento de flaky | via `sh` |
| lint | inline en `test()` | `lint_cmd`, base | no: ejecuta | ninguno | via `sh` |
| sprites | `gate_sprites` | assets | delegado a `sdlc/sprites.revisar` | juez visual en sombra | `sdlc/sprites` usa `mmorch.providers` |
| un-archivo, tipo-del-archivo | `one_file`, `_tipo_propio` | salida del coder | si, texto | recorta o descarta el codigo | no |
| aceptacion-ids, -compila, -roja | inline en `aceptacion()` | test de aceptacion | mixto: regex + ejecucion | ninguno | no |
| G1-spec-contrato | inline en `spec()` | spec | mixto | reintenta la etapa | no |
| G4-aceptacion, regresion-unidad, avance | inline en `test()` | logs de pytest, `test_counts` | mixto | escalera de fix, escala a reasoner | no |
| claude-diff-review | inline en `review()` | test de revision | ejecuta | vuelve a build | `mmorch.claude_exec` |
| usd-tope, presupuesto-razonamiento, toolchain | inline | contadores, logs | si | corta o cambia de modelo | no |

Lectura: 9 gates tienen un chequeo puro o casi puro sobre texto.
Los demas ejecutan comandos con `sh` y leen el worktree `WT`.

### Ciclo de imports (2026-10-23)

- `sdlc` importa de mmorch: `paths`, `providers`, `sandbox`, `checkers`, `impacto`, `cursor_worker`, `claude_exec`, `metrics`, `juez_sombra`.
- mmorch importa `sdlc` en 9 modulos: `auto_repair`, `hardening`, `project_repair`, `refutacion`, `juez_sombra`, `server_engine`, `server`, `sdlc_bench`, `workflow_race`.
- El encargo nombraba 5. Faltaban `server_engine`, `server`, `sdlc_bench` y `workflow_race`.
- `mmorch/checkers.py` importa solo stdlib. Es una hoja: no importa ningun modulo de mmorch ni de sdlc.
- `tests/test_capas.py` ya existe. Hoy mide alcance de modulos, no direccion de imports.

### Llamadores (union `codegraph callers` + `grep -w`, indice recien sincronizado)

- Cada `gate_*` tiene llamadores solo en `sdlc/pipeline.py` y en `tests/test_sdlc_*.py`.
- `.scratch/sdlc-6-gates/research/04-driver-v3/driver_v3.py` nombra `gate_plan_allowlist`, `gate_test_compile` y `gate_baseline`. Es research viejo.
- `_mutants` tiene llamadores en `mmorch/bughunt.py`, `mmorch/checkers.py`, `sdlc/pipeline.py` y un test.
- Conclusion: si `gate_*` queda en pipeline como envoltorio, ningun test cambia.

### Fuentes posibles de casos etiquetados

- Los tests `tests/test_sdlc_*.py` guardan incidentes reales (D4, D6, D11, SSB F7a). Cada uno es un caso con etiqueta.
- `logs/sdlc/run-log-*.json` en esta laptop: 10 corridas. Guardan la salida del gate, no la verdad.
- `logs/sdlc/veredictos.jsonl` no existe en esta laptop. Puede existir en el escritorio Windows.

## Q&A log

### Q1 — Modulo destino en mmorch
- Asked: donde viven los chequeos: (a) `mmorch/gates.py` nuevo, (b) registro en `checkers.check` con `CheckResult`, (c) paquete `mmorch/gates/`.
- Captured: el usuario eligio (a). Modulo nuevo `mmorch/gates.py`. Cada chequeo es una funcion pura con datos explicitos. Devuelve `(ok, detalle)`, el contrato de `rec_gate`. Importa solo stdlib y `mmorch.checkers`.
- Flags: ninguno.

### Q2 — Regla de direccion de imports
- Asked: (a) solo R4, (b) prohibir sdlc en todo mmorch salvo entradas, (c) R4 + R5.
- Captured (Q2): el usuario eligio (c). R6: `mmorch/gates.py` y `mmorch/checkers.py` nunca importan `sdlc`, ni directo ni transitivo. R7: ratchet; la lista de 9 modulos de mmorch que importan `sdlc` solo se achica. Ambas reglas viven en `tests/test_capas.py`.
- Flags: ninguno.

### Q3 — Orden de traslado
- Asked: tres fases (texto puro, lectura de disco, ejecucion), un gate por commit, `sin-clones` primero; alternativa: `mutacion` primero.
- Captured: el usuario confirmo el orden. Fase 1: los 9 gates de texto puro, empezando por `sin-clones`. Fase 2: `docstring-intacto`, `baseline-intacto`, `tamano-plan`; la lectura de disco queda en pipeline. Fase 3: compile, mutacion, cobertura, suite, lint. Un gate por commit. Este bd cierra con la fase 1 completa.
- Flags: la frontera chequeo/orquestacion de la fase 3 se decide en Q4 (resuelto).

### Q4 — Frontera en gates que ejecutan (fase 3)
- Asked: (a) ejecucion en `sdlc/`, decision pura sobre la salida en `mmorch/gates.py`; (b) mover tambien la ejecucion.
- Captured: el usuario eligio (a). Quedan en `sdlc/`: `sh`, sandbox, escritura de mutantes, cache de baseline. Pasan a mmorch funciones puras sobre la salida medida, por ejemplo `fallos_nuevos(log_base, log_ahora)`, `errores_compilador(log)`, `score_mutacion(muertos, total, minimo)`. Un caso etiquetado de fase 3 es un texto de log con su etiqueta, sin worktree.
- Flags: ninguno.

### Q5 — Fuente de casos etiquetados
- Asked: (a) incidentes de `tests/test_sdlc_*.py` + mutacion de artefactos con etiqueta por construccion; (b) solo run-logs con veredicto humano; (c) (a) ahora y (b) cuando haya veredictos.
- Captured: el usuario eligio (c). Ahora: incidentes reales ya guardados en tests (D4, D6, D11, SSB F7a) y casos generados por mutacion de un artefacto verde; la etiqueta sale por construccion, nunca de un LLM. Despues: se suman run-logs reales con veredicto humano posterior (`veredictos.jsonl`) cuando existan. Metrica propuesta: accuracy balanceada y tasa de falso rechazo (las del README "Measured").
- Flags: `veredictos.jsonl` no existe en la laptop; puede existir en el escritorio Windows -> usuario.

### Q6 — Medicion en este bd o en uno aparte
- Asked: bd aparte (recomendado) o (b) medir `sin-clones` en este bd junto con su traslado.
- Captured: el usuario respondio "En este bd". La medicion de precision se implementa dentro de `orchestration-bpn`, no en un bd aparte.
- Flags: alcance exacto (los 9 gates de fase 1 o solo `sin-clones`) -> Q7 (resuelto).

### Q7 — Que gates mide este bd
- Asked: (a) cada gate trasladado lleva su medicion en el mismo commit (los 9 de fase 1); (b) solo `sin-clones`.
- Captured: el usuario eligio (a). Este bd mide los 9 gates de fase 1. El primer commit traslada y mide `sin-clones` y deja el generador y el script de medicion para los 8 siguientes. Un gate sin numero medido no se marca como trasladado.
- Flags: ninguno.

### Q8 — Donde viven los casos y como se corre
- Asked: generador con semilla en `mmorch/gates_bench.py` (patron `ablation_paired.py`) o (b) casos en `tests/fixtures/gates/<gate>.jsonl`.
- Captured: el usuario confirmo el generador. `mmorch/gates_bench.py` genera los casos con semilla; sin archivos de casos. Los incidentes reales son una lista literal dentro del modulo. Corre con `python -m mmorch.gates_bench --seed N`. Cada corrida agrega una fila a `logs/ablation_results.jsonl`. README "Measured" suma una tabla con una fila por gate. `gates_bench` entra en `_ENTRADAS` de `tests/test_capas.py`. Un test chico corre el bench con pocos casos y exige un generador por gate de fase 1.
- Flags: ninguno.

## Open flags (pending input)
- Nota 2026-10-23: `tests/test_capas.py` ya usa R4 y R5 para otras reglas. Las reglas nuevas se llaman R6 (direccion) y R7 (ratchet).
- Riesgo de circularidad en la medicion: si el generador etiqueta con la misma regla que el gate, la accuracy sale 1.0 por definicion. La etiqueta sale de la intencion con que se construye el caso, no de la regla del gate -> se confirma en la propuesta del primer gate.
- Propuesta del primer traslado (`sin-clones`): el usuario dio OK y acepto el limite de 30% de edicion para etiquetar un clon.
- Q9 resuelta: el usuario eligio (a). El traslado entra igual, el numero se publica, un numero bajo abre un bd. Descartadas: (b) no trasladar bajo umbral, (c) pasar a observacion (cambia comportamiento).
- Incidentes reales sin texto original (D6 solo vive en un test reconstruido): resuelto: el usuario confirmo marcar cada incidente reconstruido en la lista literal.
- Globales a parametros (`TASK.task`, techo de `sdlc.toml`): resuelto por D1, pipeline pasa los datos.
- Al cerrar este bd: abrir bd hijos para fase 2 y fase 3 con puntero a esta captura.

## Resultado del primer traslado (2026-10-23)
- `sin-clones` paso a `mmorch.gates.sin_clones`; `sdlc/pipeline.py` solo registra con `rec_gate`.
- Medicion (`python -m mmorch.gates_bench --seed 42 --n 400`): n=401, sensibilidad 0.706, especificidad 1.00, accuracy balanceada 0.853, falso rechazo 0.
- Deteccion por tramo de edicion: 0-10% = 1.00, 10-20% = 0.97, 20-30% = 0.02. El umbral 0.8 casi no detecta copias con 20-30% editado.
- La especificidad 1.00 es una cota superior: el generador verde comparte solo imports y cabeceras.
- Resuelto 2026-10-24: el usuario pidio abrir el bd para `sin-clones` (0.853) -> `orchestration-cuz`.

## Segundo gate: spec-review (2026-10-24)
- `clarificaciones` paso a `mmorch/gates.py`. Resultado final con D12: n=287, sensibilidad 0.615, especificidad 1.00, acc. balanceada 0.807. Bd `orchestration-96p` abierto por D9.
- Medicion preliminar (seed 42, n=402): acc. balanceada 0.523, sensibilidad 0.615, falso rechazo 0.569.
- Por grupo: canonica 1.00, cero 1.00, espacios 0.00, negrita 0.00, numerada 0.00, respuesta_tbd 0.00, respuesta_vacia 0.14, sin_seccion 1.00, seccion_vacia 1.00, seis_preguntas 1.00.
- Hallazgo: una pregunta sin respuesta pasa si otra pregunta de la seccion esta bien escrita.
- Q10 resuelta: el usuario eligio (c). El bench excluye las variantes de formato (espacios, negrita, numerada) de la metrica principal y publica su acierto aparte, como costo de reintento. Con (c): n=287, sensibilidad 0.615, especificidad 1.00, acc. balanceada 0.808 (corrige un 0.86 que di sin calcular).

## Tercer gate: trazabilidad-plan (2026-10-24)
- `traza_plan` y `filas_tabla` pasaron a `mmorch/gates.py`; `_plan_files` de pipeline usa `filas_tabla`.
- Contrato de las etiquetas: el pedido de `plan()` ("cada item cita los IDs R<n> de la spec; todo R<n> aparece en algun item").
- Verdes: lista canonica, tabla (SSB F7a), rango, nombre sin carpeta (F4b); el gate los acepta a proposito, sin casos aparte.
- Rojos: ID huerfano, archivo sin ID, ID solo en prosa, ID que la spec no tiene.
- Resultado (seed 42, n=402): sensibilidad 0.52, especificidad 1.00, acc. balanceada 0.76. ID en prosa 0.00, ID inexistente 0.00.
- Bd `orchestration-1tj` abierto por D9.
- Un bug del generador (bloque vacio de IDs) daba especificidad 0.59; se corrigio y un test lo cubre.
