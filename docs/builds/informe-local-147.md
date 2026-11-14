# Informe de la Tarea 147

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 9a6a10a | 3786 | 0 | 5 |
| Final 666a530 | 3783 | 0 | 5 |

La diferencia es -3: se borraron 12 tests y entraron 9 nuevos. Ningún commit necesitó reintento del hook.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La regla de la 136 no corre en producción | Es un borrado. Guarda: los tests de lastre de la 143 siguen verdes (45 de `lastre`) | e7cc50e |
| 2 | La sonda sin resultado no avisa | `sonda_lastre_sin_resultado_llega_a_los_avisos`, `sonda_lastre_la_fila_guarda_lo_que_hizo_la_sonda` (caso `None`) | a3686df |
| 3 | `comparar_con_ancla` sin uso y carga real en `tests/` | `ruta_de_carga_obedece_root_loadavg` (leyó 7.95 en vez de 9.0) | 68da5f1 |
| 4 | Enlaces saltados, SHA del tar dentro del tar, `excluir` sin uso | `copiar_repo_rechaza_enlace_simbolico_con_uso`, `repo_con_enlace_simbolico_no_empaqueta_y_nombra_el_enlace`, `guion_de_cierre_escribe_el_sha_de_lo_que_empaqueta`, `vuelta_con_sha_del_cierre_distinto_no_se_importa`, `sha_de_la_vuelta_distinto_no_se_abre`, `sha_ausente_en_el_cierre_no_se_abre`. Guarda: `copiar_dir_copia_archivos_normales_y_filtra_secretos_y_excluidos` | 666a530 |

## 3. Qué falló o quedó incompleto

- El punto 1 toca `src/oracle/calibra.rs` (`lugar_por_de` y su test). La tabla del brief solo nombra `inyectar.rs`. El ítem 3 del punto lo exige.
- Ningún test de `tests/` llama a `medir`. Por eso no se fijó `ROOT_LOADAVG` en `tests/`. Un probe temporal lo confirmó: los 128 usos venían del binario unitario del lib.
- Falta un test de flujo para un cierre sin marcador `ROOT_SHA_VUELTA`. Lo cubre solo el test unitario con `None`.
- No corrió nada con podman. Quedan para el orquestador la suite con podman y la prueba de la 141 en Colab con `ROOT_SHA_VUELTA`.
- `MOTIVO_LASTRE_SIN_LUGAR` conserva el texto "función vieja" de la 136. Sigue en uso cuando la sonda vio funciones y ninguna está en el repo. No se reescribió: ver pregunta 3.

## 4. Lo que se borró

Todos los llamadores se unieron con `codegraph callers` y `grep -w`. Dieron lo mismo, salvo la recursión de `copiar_dir`, que solo grep ve.

Funciones y constantes en `inyectar.rs`:
- `llamados_de_tests_viejos` (solo `lastre_de_memoria`).
- `nombres_llamados_por_tests_viejos` (solo la anterior y un test).
- `es_candidata` (solo `funciones_de_efecto`).
- `es_funcion_vieja`, `llamadas_en`, `texto_de_cuerpo`, `tests_con_cuerpo` y `PALABRAS_CLAVE` (solo la regla de la 136).

Parámetros:
- `lastre_de_memoria`: `inicial`.
- `funciones_de_efecto`: `inicial` y `llamados`. Todos los llamadores pasaban `None`.
- `copiar_dir` (de `colab_calibra.rs`): `excluir`. Todas las llamadas pasaban `&[]`. La otra `copiar_dir` de `src/oracle/mod.rs` no se tocó.

Función pública en `eficiencia.rs`: `comparar_con_ancla`. Solo la usaban dos tests.

Tests borrados de `inyectar.rs` (solo probaban la regla de la 136):
- `lastre_de_memoria_pone_la_linea_en_una_funcion_del_cambio`
- `lastre_de_memoria_la_copia_del_fixture_compila`
- `lastre_de_memoria_con_dos_viejas_va_a_la_que_llama_el_test_viejo`
- `lastre_de_memoria_test_nuevo_no_hace_candidata_a_su_llamada`
- `lastre_de_memoria_sin_test_viejo_que_llame_a_una_funcion_no_aplica`
- `lastre_de_memoria_repo_chico_con_test_viejo_compila`
- `lastre_de_memoria_va_a_la_funcion_vieja_y_no_a_la_nueva`
- `lastre_de_memoria_con_el_cambio_solo_en_nuevas_va_a_una_vieja_de_otro_archivo`
- `lastre_sonda_sin_corridas_usa_la_regla_de_la_136`
- `llamados_de_tests_viejos_ignora_macros_y_palabras_clave`
- `es_funcion_vieja_sin_estado_inicial_es_siempre_vieja`
- `es_funcion_vieja_solo_si_el_texto_inicial_la_declara`

Se borraron también las constantes de fixture `TEST_LLAMA_A_VIEJA`, `VIEJA_CON_TEST` y `A_CON_TEST_NUEVO`.

## 5. Tests que cambiaron de valor

- `lastre_de_memoria_sin_ninguna_funcion_vieja_no_aplica` pasó a `lastre_de_memoria_sin_corridas_no_aplica_y_no_toca_el_repo`. Con corridas vacías espera `Ok(None)` sin tocar archivos.
- `lastre_de_memoria_con_inicial_la_copia_compila` pasó a `lastre_de_memoria_con_corridas_la_copia_compila`. Usa corridas reales del fixture.
- `lastre_de_memoria_no_elige_const_fn_ni_tests` y `lastre_de_memoria_sin_cambios_cae_en_cualquier_funcion_de_produccion` ahora pasan corridas. El primero dejó de ser trivial.
- `sonda_lastre_lugar_por_nombra_quien_eligio_el_lastre`: sin sonda espera `None`, ya no `"nombre"`.
- `sonda_lastre_la_fila_guarda_lo_que_hizo_la_sonda`: agrega el caso sin resultado.
- `ancla_sube_el_ruido_y_quita_la_falsa_regresion` pasó a `contra_ancla_el_mismo_repo_cambia_de_veredicto_con_el_ancla`. La razón es 1.04/1.05 y el ruido queda en el piso 0.02, no sube a 0.05.
- `ancla_no_tapa_una_regresion_real` pasó a `contra_ancla_regresion_real_sigue_siendo_regresion`.
- `guion_de_cierre_escribe_el_sha_de_lo_que_empaqueta`: agrega la aserción sobre el marcador.
- Doble `Falso` de `colab_calibra.rs`: el cierre imprime `ROOT_SHA_VUELTA=` con el SHA real.

Tests de otros archivos editados: ninguno. Todos los cambios caen en los cuatro archivos de la tabla.

## 6. Llamadores revisados

| Símbolo | codegraph | grep -w |
|---|---|---|
| `lastre_de_memoria` (firma) | `inyectar_con` | igual, más un literal de test en `calibra.rs` |
| `funciones_de_efecto` (firma) | `lastre_de_memoria`, `estado_global`, `efecto_en_nucleo` | igual |
| `llamados_de_tests_viejos` y `es_candidata` | un llamador cada una | igual |
| `nombres_llamados_por_tests_viejos` | `llamados_de_tests_viejos` y un test | igual |
| `comparar_con_ancla` | dos tests | igual |
| `copiar_dir` (firma) | `armar_y_empaquetar` y la guarda | además la recursión |
| `lugar_por_de` | `calibra.rs` | igual |

## 7. Preguntas para el planner

1. Un enlace llamado `.git` o `target` ahora hace fallar `empaquetar`. Antes se saltaba. Decisión: falla, porque el brief pide no saltar ningún enlace.
2. Un SHA distinto llega al usuario con el sufijo "; su paquete de vuelta no llegó", que viene de `ciclo`. Es engañoso para un SHA distinto. Decisión: no se cambia, porque el texto nombra ambos SHA y "no se abre".
3. `MOTIVO_LASTRE_SIN_LUGAR` aún habla de "función vieja". Decisión: no se reescribe, porque su test de la 135 lo espera y el brief no lo pide. Conviene reescribir texto y test juntos.
4. `lugar_por` conserva el valor `nombre` solo para leer filas viejas. Decisión: el tipo es `Option<String>`, sin cambio de esquema.
