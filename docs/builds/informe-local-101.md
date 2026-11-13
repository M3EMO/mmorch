# Informe de la Tarea 101

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh --no-fail-fast`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 4a6ef27 | 3171 | 0 | 4 |
| Final | 3188 | 0 | 4 |

Ningún hook de commit falló por un test de tiempo. Ningún commit se reintentó.

## 2. Commits

| Grupo | Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|---|
| B | B1 | cargo-mutants corría los "delete field" | `mutacion_listado_y_corrida_excluyen_los_delete_field` | 18c60a7 |
| B | B2 | el veredicto no leía `fuera_de_la_muestra` | `oraculo_mutantes_de_mas_dan_el_aviso_en_el_veredicto` | 8897927 |
| B | B3 | la regla de producto roto no tenía clave | `informe_no_compila_sin_bloqueos_da_no_listo_con_su_disparo`, `oraculo_no_compila_contrato_deja_funciona_en_cero_y_ocultos_fallados` | dbbc1fa |
| C | C2 | `cuerpo_vaciado` exigía estado inicial | `cuerpo_vaciado_sin_inicial_aplica_en_un_repo_de_fixture` | 72dca1e |
| C | C1 | Completitud ofrecía `modulo_borrado` | `de_dimension_completitud_no_trae_modulo_borrado` | 1e500fc |
| C | C4 | `!x == 1` no compila | `agregar_negacion_envuelve_la_condicion_con_parentesis` | 8d1932e |
| C | C5 | `nombre_de_funcion` no veía calificadores | `nombre_de_funcion_reconoce_calificadores_y_visibilidad` | 8323e9a |
| A | A1 | un caso que no compila contaba como detección | `rompe_la_compilacion_funciona_y_completitud_con_marcador_nuevo_es_verdadero`, `..._con_no_compila_nuevo_es_verdadero` | 508e3b6 |
| A | A2 | cambios explicados por los tests apagados contaban como ruido | `separar_cambios_muerto_a_vivo_con_asesinos_apagados_no_es_ruido` | 66c4bbb |
| A | A3 | Pruebas ignoraba `con_tests/` | `calibra_pruebas_usa_con_tests_de_la_tarea_y_la_fila_lo_dice` | 5008205 |

C2 va antes que C1: el hook corre la suite completa y dos tests de Completitud dependen de C2.
Los commits de A2 y A3 regeneran `docs/interfaces.md`, porque el hook lo exige.

## 3. Qué falló o quedó incompleto

- **C3 sin hacer.** `analizar` (`perfiles/rust.rs`) no ve comentarios de bloque ni cadenas crudas. Su línea `limpia` no conserva posiciones en bytes. Arreglarlo exige tocar `perfiles/rust.rs`, fuera de la tabla. El test en rojo quedó en `/home/mpera/.cache/root-wt/logs-local101c/inyectar-c3-rojo-sin-commit.rs`. `linea_limpia` sigue en uso.
- **C1.2 sin hacer.** `modulo_borrado` sigue: `Defecto::ModuloBorrado` la llama y `defectos.rs` y `calibra.rs` usan la variante.
- **B1.** Nadie probó el patrón contra la salida real de cargo-mutants 27.1.0. Lo prueba el orquestador en la corrida real.
- **A3.** `tests/calibra.rs` recibió el campo nuevo en tres literales, solo para compilar.

## 4. Tests que cambiaron de valor

- `rompe_la_compilacion_completitud_y_funciona_son_falsas` pasó a `..._con_marcador_nuevo_es_verdadero`: A1.
- `calibra_completitud_con_no_compila_mide_y_no_reemplaza` pasó a `..._reemplaza_y_no_mide`: A1.
- `calibra_caso_guardado_viejo_carga_sin_los_campos_de_mutacion` quita también `mutantes_explicados`: A2.
- `mutacion_sin_acotar_los_comandos_son_iguales_a_hoy` y `mutacion_con_plazo_fijo_el_comando_lleva_timeout_antes_de_output`: el comando lleva `--exclude-re`.
- `informe_no_compila_sin_bloqueos_da_no_listo_con_su_disparo`: compara el motivo completo con la clave.
- `inyectar_de_dimension_lista_sus_defectos_y_funciona_tiene_el_suyo`: Completitud da solo `CuerpoVaciado`.
- `cuerpo_vaciado_sin_inicial_no_aplica` pasó a `..._aplica_y_marca_si_cruza_el_cambio`: C2.
- `comportamiento_con_inicial_if_ofrece_or_y_negacion_de_la_condicion`: `if !x && y` pasó a `if !(x && y)`: C4.

## 5. Funciones borradas

Ninguna. Se repuso `aviso_de_mas` en `mutacion.rs`.

## 6. Llamadores revisados

- `rompe_la_compilacion`: firma igual. Los llamadores están en `calibra.rs`.
- `mutantes_cambiados`: firma igual. Los llamadores de `calibra.rs` pasan por `separar_cambios`.
- `modulo_borrado`: `inyectar_con` (codegraph y grep -w). No se borró.
- `linea_limpia`: `cuerpo_de` y `candidatos_de_linea`. No se borró.
- `cuerpo_vaciado` (ahora `(Lugar, bool)`) y `ediciones_de_condicion`: funciones privadas con un llamador cada una.
- `nombre_de_funcion`: `en_camino`, `cuerpo_vaciado` y `apagar_tests`.
- `de_dimension`: dos llamadores en `calibra.rs` y `item_y_lugar_de`. Cambió el contenido de Completitud.
- `repo_de`: un solo llamador, `calibrar_tarea_con`.
- `Fila`: todos los literales de `src/` y `tests/` reciben `repo_origen`.

## 7. Preguntas para el planner

1. **C3.** ¿Se cambia `perfiles/rust.rs` en un punto aparte? Decisión: no se hizo, y no se duplicó el lector.
2. **`Defecto::ModuloBorrado`.** ¿Se quita la variante? Decisión: solo salió de `de_dimension`.
3. **Build roto por la suite oculta.** Esa ruta no marca `no_compila` (`mod.rs:761`), así que A1 no la ve. Decisión: no se cambió.
4. **Estado `Vencido`.** Un mutante muerto que pasa a `Vencido` cuenta como ruido. Decisión: ruido.
5. **Sin asesinos conocidos.** Un cambio de un mutante sin `tests` en la base es ruido. Decisión: conservador.
6. **`--repo` y `con_tests/` juntos.** Gana `con_tests/` sin aviso. Decisión: el brief lo pide así.
7. **Campo de la fila.** `repo_origen` guarda `"con_tests"`, o `None`. El brief no fijó el nombre.
8. **Plural.** El aviso dice "corrió 1 mutantes" con N=1. Decisión: plantilla fija del brief.
9. **Aviso "sin juez: el producto no compila".** No cambió. Conviene unificarlo después.
10. **`bucle`.** Sigue con su detección propia, que excluye `const fn`, `async fn` y `unsafe fn`. Decisión: C5 habla solo de `nombre_de_funcion`.
11. **`if` sin `{` en la línea.** C4 no ofrece candidato. Decisión: no envolver.
