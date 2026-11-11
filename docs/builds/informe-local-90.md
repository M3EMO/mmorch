# Informe de la Tarea 90

La rama es `laptop/tarea-90`, sobre af2ab61. Todo corrió con `ROOT_SIN_PODMAN=1`.
Cada parte la construyó un subagente Haiku en su propio worktree. El organizador revisó cada diff y commiteó.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (af2ab61) | 2990 | 0 | 4 |
| Final | 3015 | 0 | 4 |

- El hook de commit no falló en ningún punto. No hizo falta reintentar.
- La suite con podman, la validez, `corre`, `calibrar` y la medición de broot no corrieron. Las corre el orquestador.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 5 | `root_registrar` marcaba `origen` solo en `plan-aprobado`; el resumen contaba eventos MCP como humanos | `mcp_unit_humano_por_mcp_lleva_origen_mcp`, `observe_humano_por_mcp_despues_de_plan_no_cuenta_como_humano`, `observe_plan_aprobado_por_mcp_no_fija_el_plan`, `observe_cuenta_aparte_los_declarados_por_mcp` | 679b1ca |
| 2 | `refusal` y `content_filter` pasaban como respuesta completa | `proveedores_anthropic_stream_refusal_es_truncada`, `proveedores_deepseek_stream_content_filter_es_truncada_aunque_venga_vacia` | 7eb89c3 |
| 3 | `aplicar_redaccion` aplicaba textos con `zip` aunque la cantidad no coincidiera | `informe_redactor_con_menos_textos_que_visibles_conserva_los_originales_y_avisa`, `informe_redactor_con_menos_proximos_pasos_que_visibles_conserva_los_originales_y_avisa` | 31ef421 |
| 4 | `aprende::leer_eventos` no tomaba el lock común | `leer_eventos_toma_el_lock_antes_de_listar_los_archivos` | c0c8e1a |
| 6 | `inestables_de` ignoraba `tarea_hash` | `inestables_de_con_otro_tarea_toml_no_los_da` | e01b85a |
| 7 | `pub(in crate::x) use` contaba `x` como destino | `aristas_pub_in_no_cuenta_el_camino_de_visibilidad`, `aristas_pub_in_agrupado_no_cuenta_la_visibilidad`, `sonda_arquitectura_pub_in_no_inventa_ciclo` | c9d5286 |
| 1 | Escenarios y build sin límites; un `RUN --network` no se frenaba | `entrega_args_build_llevan_capacidades_y_memoria_sin_pids_limit`, `entrega_arranque_de_escenario_lleva_capacidades_pids_y_memoria`, `entrega_dockerfile_con_run_network_frena_antes_del_build`, `entrega_run_network_en_minusculas_y_con_espacio_frena` | 91f3eb2 |

Guardas que pasaron desde el inicio:
- Punto 1: `entrega_dockerfile_sin_network_construye_como_hoy`.
- Punto 2: los tests existentes de `end_turn` y `stop`.
- Punto 3: `informe_redactor_con_la_misma_cantidad_aplica_sus_textos_y_no_avisa`.
- Punto 5: `observe_humano_sin_origen_despues_de_plan_sigue_contando`.
- Punto 6: `inestables_de_con_mismo_tarea_toml_los_da`.
- Punto 7: `aristas_otras_visibilidades_y_use_simple_siguen_igual`.

## 3. Qué falló o quedó incompleto

- **Punto 1, guarda con podman.** `guarda_escenario_sin_ping_ni_procesos_de_mas` no corrió, porque `ROOT_SIN_PODMAN=1` la saltea. Nadie verificó que `ping` falle en el host ni que `sh` salga con error al fallar un `fork`. El orquestador debe correrla una vez con podman.
- **Punto 1, build con `--cap-drop=all`.** No se probó contra un build real. Un `RUN apt-get install` puede necesitar capacidades como `CAP_CHOWN` o `CAP_SETUID`. Si falla, el orquestador lo verá en la pasada con podman.
- **Punto 5, "declarados por MCP" no aparece en la salida de `observa resumen`.** Existe la función pura `observe::contar_declarados_mcp`. Mostrar el conteo exige un campo en `Resumen` (`src/tipos.rs`) y su impresión, fuera de la lista de archivos.
- **Punto 5, campo `origen`.** No hay campo tipado nuevo. Ver la pregunta 1.
- **Punto 7, medición sobre broot.** No corrió. Ningún test del repo usa broot. La cifra de 219 aristas viene de `docs/builds/informe-local-89.md`, sección 5. Para repetirla: extraer el commit `1f77962e6ff3` de `Canop/broot` con `git archive` y correr `r00t oraculo capas --repo <dir> --json`. Se espera un número igual o menor.
- **Punto 7, residuo.** Una línea que no es `use`, como `pub(in crate::a) fn f() {}`, sigue contando `a`. La rama de líneas sin `use` no pasa por `sin_visibilidad`.
- **Punto 4, error de línea inválida.** Para conservar el mensaje, `aprende` vuelve a buscar la línea inválida solo cuando falla. Si una rotación la mueve entre las dos lecturas, sale `ErrorAprende::Json` sin archivo ni número.
- **Punto 3, plural.** El aviso dice "1 textos" cuando N es 1. Es el texto exacto del brief.

## 4. Opciones de `podman build`

Versión medida: podman 5.8.9 en el host. No se midió la versión dentro de la imagen.

| Opción | `podman build` la acepta | Decisión |
|---|---|---|
| `--cap-drop` | Sí | Se agrega como `--cap-drop=all` |
| `--security-opt` | Sí | Se agrega como `--security-opt=no-new-privileges` |
| `--memory` | Sí | Se agrega con la constante `MEMORIA` |
| `--pids-limit` | No | No se agrega |

## 5. Tests que cambiaron de valor

- `entrega_red_args_sin_credenciales_ni_montajes` (`src/oracle/entrega.rs`): fija la lista exacta de `args_build`. Ahora incluye las tres opciones nuevas, con red y sin red.
- `informe_redaccion_con_ids_reemplaza_el_texto_de_la_accion` (`src/oracle/informe.rs`): mandaba 1 texto para 3 acciones visibles y dependía del `zip` defectuoso. Ahora manda 3.
- `mcp_registrar_escribe_el_evento_y_el_resumen_lo_cuenta` (`tests/mcp.rs`): el plan y el humano vienen por MCP. `intervenciones` pasa de 1 a 0.
- Tests de `inestables_de` en `calibra.rs`: pasan a la firma nueva con un `tarea_hash` actual.

## 6. Llamadores revisados

| Símbolo | Cambio | codegraph | grep -w |
|---|---|---|---|
| `Calibraciones::inestables_de` | gana el parámetro `tarea_hash` | `excluidos_de` (`mod.rs:1571`) | `mod.rs:1586` y los tests de `calibra.rs` (ya actualizados) |
| `excluidos_de` | gana el parámetro `tarea_hash` | `medir` (`mod.rs:989`) | las dos llamadas dentro de `medir` y la definición |
| `aprende::leer_eventos` | cuerpo nuevo, misma firma | `lib.rs:770` (`aprende_accion`) | `lib.rs:783`, `tests/aprende.rs:226` y `:371` |
| `observe::uso::leer_eventos` | sin cambio, ahora lo usa `aprende` | `uso.rs`, `corrida.rs`, `observe/mod.rs:62` | además `observe/claude.rs`, `proveedores/mod.rs:1262`, `tests/calibra.rs:296` |
| `PIDS_LIMITE`, `MEMORIA` | pasan a `pub(crate)`, sin moverse | no aplica | los usa `entrega.rs` |

- Existe otra función `inestables_de` en `calibra/inestables.rs`. No cambió. Sus llamadores son otros.
- Ningún símbolo se borró. `aprende::leer_eventos` queda como envoltorio, porque tiene llamadores.
- `medir` ya devolvía `Result`. Su firma no cambió.

## 7. Preguntas para el planner

1. **Campo `origen` del evento.** El brief pide un campo nuevo con `#[serde(default)]`. Ese campo vive en `src/tipos.rs`, fuera de la lista. Decisión: se usa `extra["origen"] = "mcp"`, que `root_registrar` ya ponía en `plan-aprobado`. Ahora lo pone en todo evento. El esquema no cambia. `docs/interfaces.md` suma un párrafo. ¿Se prefiere un campo tipado?
2. **Resumen "declarados por MCP".** Decisión: solo la función `contar_declarados_mcp`. El campo en `Resumen` y su impresión quedan para una tarea que toque `src/tipos.rs` y la CLI.
3. **Un plan por MCP.** Decisión: un `plan-aprobado` con origen MCP no fija el plan aprobado. Un humano posterior no cuenta como intervención. ¿Se prefiere otra regla?
4. **`inestables_de` y filas nuevas.** Decisión: se usa la fila más reciente que sea vigente y del mismo `tarea_hash`. Una fila nueva de otro `tarea.toml` no tapa a una vieja válida. `aplicar` y `motivo_sin_calibrar` siguen con la más reciente sin filtrar. Las dos reglas pueden divergir en ese caso.
5. **`--cap-drop=all` en el build.** El brief lo pide. Puede romper un `RUN` que cambie dueños o ejecute `setuid`. ¿Se acepta ese costo?
6. **Redactor con listas vacías.** Un redactor que devuelve una lista vacía, con acciones visibles, produce un aviso por lista. Decisión: se acepta, porque es la regla pedida.
