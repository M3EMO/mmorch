# Informe de la Tarea 118

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (e19886e): la suite de la biblioteca dio 2820 verdes, 0 fallas y 1 ignorado. No sumé los demás binarios.
- Final: 3398 verdes, 0 fallas y 5 ignorados, sumados todos los binarios.
- La diferencia de la biblioteca es de 3 tests, los nuevos del punto 1.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `barrer_al_arrancar` llamaba a podman con `ROOT_SIN_PODMAN=1` | `barrido_con_sin_podman_uno_no_llama_al_programa` | 6a7938f |
| 2 | Auditoría, sin código | No aplica | Este informe |

Guardas del punto 1:

- `barrido_sin_variable_llama_al_programa`
- `barrido_con_otros_valores_de_sin_podman_llama_al_programa`
- Los tests de barrido de las Tareas 107, 107b y 112.

`barrer_al_arrancar` conserva su firma. Lee la variable y delega a `barrer_al_arrancar_con(programa, Option<&str>)`. Los tests inyectan el valor. `barrer`, `contenedor.rs` y `entrega.rs` no cambiaron.

## 3. Qué falló o quedó incompleto

- `barrido_con_otros_valores_de_sin_podman_llama_al_programa` falló una vez en cinco corridas con `Text file busy` (ETXTBSY). Otro hilo hace fork mientras `podman_falso` escribe el script. El helper es de otros tests y no lo cambié. La suite final pasó entera.
- `podman_falso` con lista vacía escribe un error de sintaxis de `sh` en stderr. No afecta los resultados.
- Los tests con podman real quedan sin probar, como pide el brief. La prueba de convivencia queda para el orquestador.
- La auditoría usó un podman falso con salida vacía. Un camino que depende de la salida puede cortarse antes. Puede faltar algún test.
- No usé codegraph en la auditoría. Usé grep y la corrida dinámica.

## 4. La auditoría

Método: la suite corrió con un `podman` falso al frente del `PATH` y `--test-threads=1`. Cada llamada se atribuyó al test que terminó después. Hubo 216 llamadas. Ningún test con `saltar_sin_podman` llamó al falso.

Hallazgo: `calibrar_tarea_con` lee el digest de la imagen en `calibra.rs:1578`. Esa lectura corre `podman image inspect` (`calibra/target.rs:52`) aunque el test inyecte su evaluador. Por eso 41 tests de calibración llegan a podman real.

Camino de los 41: test -> `calibrar_tarea_con` (calibra.rs:2088) o `calibrar_tarea` (calibra.rs:1876) -> `leer_digest` -> `target::digest_de_imagen`.

Tests de `src/oracle/calibra.rs` (28):

- calibra_limpios_reusados_dan_los_mismos_casos_que_sin_cache
- calibra_diseno_no_reusa_un_cache_sin_su_puntaje_y_lo_sobrescribe
- calibra_diseno_no_reusa_un_limpio_que_arquitectura_no_midio_en_diseno
- calibra_diseno_con_cache_corrupto_reevalua
- calibra_diseno_con_otra_version_del_repo_no_reusa_nada
- calibra_un_caso_limpio_da_la_misma_especificidad_con_y_sin_reuso
- calibra_diseno_marca_limpio_reusado_solo_en_los_limpios_del_cache
- calibra_diseno_reusa_los_limpios_que_arquitectura_ya_juzgo
- calibra_diseno_tras_arquitectura_con_evaluador_real_no_llama_al_juez_en_limpios
- calibra_tarea_escribe_una_fila_nueva_y_conserva_las_anteriores
- calibra_el_caso_guardado_dice_su_valor_y_el_de_la_base
- calibra_seguridad_no_escribe_ni_lee_la_carpeta_de_limpios
- calibra_carpeta_temporal_queda_dentro_de_home_tmp
- calibra_evalua_sin_historial
- calibra_pruebas_con_evaluar_con_real_aplica_el_defecto_en_la_muestra_repartida
- calibra_pruebas_el_caso_con_assert_borrado_corre_acotado_al_mutante
- calibra_pruebas_el_renglon_de_assert_borrado_guarda_mutante_y_tests
- calibra_pruebas_los_muertos_de_la_base_llegan_al_inyector
- calibra_repo_de_tarea_se_resuelve_desde_su_carpeta
- calibra_un_caso_de_pruebas_corre_la_mutacion
- calibra_pruebas_assert_borrado_sin_muertos_dice_que_la_base_no_dio_muertos
- calibra_pruebas_assert_borrado_sin_tests_ubicables_de_muerto_dice_que_ninguno_tiene_tests_ubicables
- calibra_pruebas_con_con_tests_sin_repo_cli_no_avisa
- calibra_pruebas_con_repo_cli_sin_con_tests_no_avisa
- calibra_pruebas_sin_con_tests_la_fila_no_lo_dice
- calibra_pruebas_usa_con_tests_de_la_tarea_y_la_fila_lo_dice
- calibra_un_caso_de_diseno_toma_la_mutacion_de_la_base
- calibra_un_caso_limpio_toma_de_la_base_las_sondas_deterministas_y_corre_el_juez_y_los_tests

Tests de `tests/calibra.rs` (13):

- calibra_casos_dos_calibraciones_no_se_pisan
- calibra_tarea_dos_corridas_agregan_filas_y_no_borran_las_anteriores
- calibra_tope_incompleta_con_la_misma_semilla_se_repite
- calibra_casos_defecto_sin_veredicto_guarda_el_motivo_y_no_detecta
- calibra_casos_detencion_por_tope_guarda_los_casos_hechos
- calibra_casos_guarda_un_renglon_por_caso_medido_en_orden
- calibra_tarea_evalua_la_base_y_cada_caso_con_las_partes_de_su_dimension
- calibra_tope_detiene_antes_del_caso_que_no_entra
- calibra_tope_suficiente_da_fila_completa_y_sin_gate
- calibra_tope_tras_la_base_sin_casos_no_escribe_fila
- calibra_casos_inyeccion_invalida_reemplazada_queda_en_el_renglon
- calibra_casos_defecto_no_aplicable_deja_renglon_y_no_suma_a_n
- calibra_casos_todos_no_aplicables_escriben_el_archivo_con_sus_renglones

Otros dos tests, que llegan por `ps` y `run`:

- `panico_en_entrada_lo_detecta_la_sonda_nativa` (`src/oracle/calibra/defectos.rs:309`). Camino: `evaluar_copia_con` (defectos.rs:117) con `PodmanReal`.
- `oraculo_home_evaluar_y_llamadas_escriben_solo_en_su_home` (`tests/oraculo_home.rs:55`). Camino: `evaluar` (mod.rs:455) con `PodmanReal`.

Estado de cada lugar con el literal `"podman"`:

| Lugar | Tests sin guarda que lo alcanzan |
|---|---|
| contenedor.rs:282 (`correr_podman`) | Vía `PodmanReal::exec`, sin atribución propia |
| contenedor.rs:362 (`lanzar_hijo`) | Ninguno |
| contenedor.rs:444 (`lanzar`) | `panico_en_entrada...`, `oraculo_home...` |
| contenedor.rs:697 (`terminar`) | Ninguno |
| descarga.rs:1305 | Ninguno, sus tests tienen guarda |
| entrega.rs:208 (`PodmanReal::correr`) | `panico_en_entrada...`, `oraculo_home...` |
| calibra/target.rs:52 | Los 41 de calibración |
| corre/ejecutor.rs:242 | Ninguno |
| corre/corrida.rs:796 | Ninguno, solo actúa con una señal |

El barrido (`barrer_al_arrancar`) ya no llama a podman con la variable en `1`. Los tests de `oraculo_home` y `panico_en_entrada` ya no hacen `ps` por el barrido. Siguen llegando a `run` e `image inspect`.

Nota: una guarda dentro de un helper solo sale del helper. El test que lo llama sigue y puede llegar a podman.

## 5. Tests que cambiaron de valor

- Ningún test cambió de valor.
- No edité tests de otros archivos.

## 6. Llamadores revisados

La firma pública de `barrer_al_arrancar` no cambió.

- grep -w: `contenedor.rs:225` y `entrega.rs:1613`.
- codegraph: no lo corrí, porque la firma no cambió.
- `barrer_al_arrancar_con` es nueva y solo la llama `barrer_al_arrancar` y sus tests.

## 7. Preguntas para el planner

1. Los 43 tests de la sección 4 llegan a podman real sin guarda. ¿Los guardamos con `saltar_sin_podman`, o inyectamos la lectura del digest en `calibra.rs:1578`? Decisión del organizador: no tocarlos, porque la tabla de archivos del brief no lo permite.
2. ¿Corregimos la carrera ETXTBSY de `podman_falso` con un reintento? Decisión: no tocarla. Es un helper compartido y la falla es rara.
