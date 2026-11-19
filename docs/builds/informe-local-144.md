# Informe de la Tarea 144

La Tarea 144 cambia solo `src/oracle/eficiencia.rs`. Cada medida de tiempo de un caso usa una referencia medida en el mismo momento. Un caso con la máquina cargada no da veredicto.

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (punta de `laptop/tarea-143`) | 3679 | 0 | 5 |
| Final | 3692 | 0 | 5 |

`cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` salen limpios.
Ningún test de tiempo falló en el hook. No hubo reintentos de commit.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `tiempo_tests` y `memoria_pico` de un caso usaban la serie de la base y una sola ancla | `con_referencia_el_inicial_corre_sus_tests_corridas_tests_veces` (1 contra 10); `con_referencia_el_doble_en_ambos_lados_da_razon_uno_y_sin_regresion` (razón 2); `con_referencia_los_tests_alternan_repo_e_inicial_en_cada_par`. Guarda: `sin_referencia_el_orden_alterna_como_hoy` | 290bbd0 |
| 2 | La razón de `tiempo_compilacion` salía de la serie de la base | `con_referencia_la_compilacion_se_compara_contra_su_ancla` (razón 2 en vez de 1,015); `con_referencia_ancla_de_compilacion_con_error_deja_la_medida_sin_valor`. Guarda: `con_referencia_compilacion_al_doble_del_ancla_es_regresion` (verde desde el inicio) | fc54b9b |
| 3 | Ningún control veía la máquina cargada | `caso_con_carga_alta_deja_tiempos_sin_valor_con_su_motivo`; `la_lectura_despues_de_las_corridas_tambien_cuenta`; `base_cargada_deja_la_calibracion_sin_valor_y_el_caso_hereda_el_motivo`; `lectura_ilegible_deja_tiempos_sin_valor_con_motivo_de_lectura`. Guardas: `carga_baja_no_cambia_ninguna_medida`, `carga_alta_no_quita_memoria_ni_tamano`, `carga_de_exactamente_la_mitad_de_los_nucleos_no_es_carga` | e1ed4a0 |

## 3. Qué falló o quedó incompleto

- **Calibración de Eficiencia de t1 a t3:** no se corrió. Necesita podman y una laptop sin otro trabajo. La corre el orquestador.
- **`comparar_con_ancla`:** es `pub` y ya no la usa código de producción. Solo la usan dos tests. Se dejó sin borrar para no cambiar la API pública.
- **Tests de integración en `tests/`:** el control de carga se apaga solo bajo `cfg(test)` del crate. Un test de `tests/` que llame a `medir` con ejecutores falsos lee el `/proc/loadavg` real. Ninguno falló en la suite.
- **Tests con podman:** ninguno nuevo necesita `saltar_sin_podman`.

## 4. Las llamadas de un caso

Un caso usa la referencia fijada y `CORRIDAS_TESTS` = 10. La metadata y las listas del lado inicial salen de la referencia.

**Lado repo, en orden:**
1. `metadata`: 1 corrida.
2. `lista`: 1 corrida.
3. `compilar_tests`: 1 corrida.
4. `tests`: 10 corridas, alternadas con el lado inicial.
5. `build`: 1 corrida.

**Lado inicial, en orden:**
1. `compilar_tests`: 1 corrida.
2. `tests`: 10 corridas, alternadas con el repo.
3. `build`: 1 corrida, el ancla de la compilación.

Orden de los tests: repo, inicial, repo, inicial, y así 10 veces. Orden del build: repo, luego inicial.
El control de carga lee `/proc/loadavg` antes de las corridas medidas y después de ellas.
La medición de la base (sin referencia) no cambia: cada lado corre 10 tests y 3 builds, alternados, con el inicial primero.

## 5. Tests que cambiaron de valor

Todos están en `src/oracle/eficiencia.rs`.

| Test | Cambio | Motivo |
|---|---|---|
| `con_referencia_el_inicial_solo_recibe_su_ancla` | Renombrado a `..._no_pide_metadata_ni_lista`. Espera 10 tests | El inicial corre sus tests en cada caso |
| `con_referencia_el_inicial_recibe_una_llamada_por_medida_de_tiempo` | Renombrado a `..._compila_una_vez_y_una_su_ancla`. Ya no cuenta tests | Lo cubre el test nuevo de 10 corridas |
| `con_referencia_el_orden_de_las_corridas_es_repo_inicial_repo_repo` | Renombrado a `..._el_orden_de_las_compilaciones_es_repo_inicial` | El orden de tests lo cubre el test nuevo |
| `con_referencia_un_ancla_con_error_se_ignora` | Renombrado a `..._no_toca_los_tests` | Ya no hay ancla de tests. Un ancla de compilación con error deja la medida sin valor |
| `con_referencia_el_ancla_con_error_deja_aviso_por_medida` | Renombrado a `..._deja_aviso_de_compilacion`. Espera un solo aviso | No hay ancla de `tiempo_tests` |
| `con_referencia_la_razon_usa_la_serie_guardada` | Tiempo y memoria salen del inicial vivo. La compilación usa el ancla (razón 4/9) | Reglas nuevas de los puntos 1 y 2 |
| `referencia_con_serie_fallida_deja_la_medida_sin_valor_con_el_motivo_de_la_base` | Renombrado a `con_referencia_serie_viva_fallida_...`. Falla el inicial vivo | La memoria ya no sale de la base |
| `con_referencia_corridas_referencia_son_la_serie_mas_el_ancla` | Renombrado a `..._son_las_vivas_del_inicial`. Espera 10 corridas | Sin ancla de tests |
| `ancla_entra_a_la_mediana_de_la_referencia` | Borrado | Probaba `comparar_con_ancla_por_mediana`, que se quitó por falta de uso |

Los tests de otros archivos no cambiaron.

## 6. Preguntas para el planner

1. **Lectura de carga que falla en producción.** Decisión: las medidas de tiempo quedan sin valor con el motivo "no se pudo leer la carga". No hay respaldo silencioso. La alternativa es seguir sin control.
2. **Carga justo en la mitad.** Decisión: 4,00 con 8 núcleos no es carga. Solo una carga mayor que la mitad lo es.
3. **Cuál lectura se informa.** Decisión: el motivo muestra la mayor de las dos lecturas, con dos decimales.
4. **Base cargada.** Decisión: `Referencia` guarda el motivo en un campo nuevo, privado y sin cambio de esquema. Los casos que la usan heredan el motivo.
5. **Compilación con ancla que falla.** Decisión: la medida queda sin valor. El aviso de la Tarea 135 cambia su texto, porque la referencia ya no sigue con la base.
6. **Orden de los pares.** Decisión: el repo corre primero en cada par de un caso, como pide el brief. La base conserva el inicial primero.
7. **Casos cargados corren igual.** Decisión: un caso cargado en la primera lectura igual corre sus mediciones, porque la lectura posterior también cuenta. Cuesta unos 80 s de compilación perdidos.
