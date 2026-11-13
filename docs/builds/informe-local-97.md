# Informe de la Tarea 97

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (7b65814): 3115 verdes, 0 fallas, 4 ignorados. La cifra sale del informe de la Tarea 96.
- Final: 3135 verdes, 0 fallas, 4 ignorados. Son 20 tests nuevos.
- El hook de commit no falló por tiempo. No hubo reintentos.

## 2. Plazo de cargo-mutants

- **Duración de la base.** Sale del `outcomes.json` de la base: el `duration` de la fase `Test` del escenario `Baseline`, en `phase_results`. Se lee en segundos.
- **Multiplicador.** 5, con mínimo de 20 segundos. El plazo es `max(20, ceil(5 × duración))`.
- **Fuente.** https://mutants.rs/timeouts.html: "The default test timeout is 5 times the baseline test time, with a minimum of 20 seconds."
- **Sin confirmar.** Esa página no documenta `phase_results` ni la unidad de `duration`. Los segundos en `f64` salen de memoria del formato de cargo-mutants. Ningún cargo-mutants real corrió en esta laptop.
- **Sin duración.** Si el reporte no trae la duración, los casos usan el plazo automático. La fila lo muestra con `plazo_mutantes = None`.

## 3. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Los casos no guardaban el estado de cada mutante | `mutacion_muestra_da_muerto_vivo_y_vencido_en_orden`, `mutacion_sonda_deja_su_muestra_y_tomar_muestra_la_saca`, `mutacion_mutantes_cambiados_omite_el_elegido_y_lista_el_resto`, `calibra_caso_con_un_mutante_que_cambia_queda_ruidoso`, `calibra_fila_cuenta_los_casos_ruidosos_sin_mover_las_metricas`. Guardas: `mutacion_puntaje_no_cambia_con_el_estado_vencido`, `calibra_caso_guardado_viejo_carga_sin_los_campos_de_mutacion`, `calibra_fila_vieja_sin_casos_ruidosos_lee_cero` | 3038131 |
| 2 | El plazo de cada mutante bajaba al apagar tests | `mutacion_plazo_de_base_multiplica_con_piso_y_redondea_hacia_arriba`, `mutacion_parsear_outcomes_lee_la_duracion_de_test_del_baseline`, `mutacion_sonda_deja_la_duracion_de_base_y_tomar_la_saca`, `mutacion_con_plazo_fijo_el_comando_lleva_timeout_antes_de_output`, `mutacion_sin_plazo_fijo_el_comando_no_lleva_timeout`, `calibra_casos_corren_con_plazo_fijo_y_la_base_sin_el`. Guarda: `calibra_fila_vieja_sin_plazo_mutantes_lee_none` | 9e5d105 |
| 3 | La muestra caía fuera del cambio | `dentro_del_cambio_incluye_los_bordes_y_descarta_lo_de_afuera`, `mutacion_con_cambios_filtra_la_corrida_a_los_mutantes_dentro`, `mutacion_con_cambios_sin_mutantes_dentro_da_sin_veredicto`, `mutacion_con_cambios_mas_mutantes_que_el_tope_reparte_la_muestra_dentro`. Guarda: `mutacion_sin_cambios_es_igual_a_sonda_mutacion` | a1f54f9 |

Cada punto lo construyó un subagente Haiku en el worktree único, en serie, porque los tres tocan `mutacion.rs`. El organizador revisó cada diff antes del commit.
Los tests en rojo se vieron fallar por aserción, con tipos y funciones en stub.

## 4. Qué falló o quedó incompleto

- Ningún punto quedó sin hacer.
- Nada se probó contra un cargo-mutants real. El flag `--timeout` y el campo `phase_results` quedan para el orquestador.
- Fuera de la tabla del brief se tocaron solo literales de `Fila` para compilar, en `src/oracle/mod.rs` y `tests/calibra.rs`. El punto 3 toca `mod.rs` en una llamada de `medir`, como permite el brief.
- Un caso con veredicto compartido (`limpio_reusado`) guarda las listas vacías. No corrió la sonda.
- Un mutante que está solo en la base o solo en el caso no cuenta como cambiado. Un defecto que corre líneas cambia el nombre del mutante y lo oculta.
- Los rangos se calculan por copia en `medir`. Con `bootstrap/`, la muestra de un caso puede variar un poco frente a la base.

## 5. Tests que cambiaron de valor

Ninguno. Los literales de `Fila` suman `casos_ruidosos: 0` y `plazo_mutantes: None`, sin cambiar sus aserciones.

## 6. Llamadores revisados

Codegraph y `grep -w` coinciden.

- `evaluar_copia` (firma cambiada, ahora devuelve la muestra): `medir_caso` (limpio y defecto) y `la_suite_no_la_ejecuta`.
- `sonda_mutacion`: firma igual. Llama a `sonda_mutacion_en`. Su único llamador de producción es `medir` en `mod.rs`, que ahora llama a `sonda_mutacion_en`. Los tests de `mutacion.rs` siguen con la firma vieja.
- `tomar_muestra`, `tomar_base_segundos`, `mutantes_cambiados`, `con_plazo_fijo`: símbolos nuevos. Los usan `evaluar_copia`, `calibrar_medida_con_hilos` y `medir_caso`.
- `comando_de_corrida`: sin `--timeout` el comando es idéntico. `es_comando_de_mutacion` lo reconoce igual.
- Literales de `Fila` y de `ResultadoMutacion`: arreglados en `calibra.rs`, `mod.rs` y `tests/calibra.rs`.

## 7. Preguntas para el planner

1. **La base sin duración.** Si el reporte no trae `phase_results`, la calibración cae al plazo automático sin marcar la fila. Decisión: solo muestra `plazo_mutantes = None`. No marca `incompleta`.
2. **Bootstrap sin cambios.** Con `bootstrap/` y sin mutantes en el cambio, Pruebas queda sin veredicto, como pide el brief. Eso incluye un repo igual al inicial. Decisión: se cumple el brief tal cual.
3. **Total con cambios.** Con rangos, `total_conocido` y `truncado` cuentan los mutantes dentro del cambio, no los del listado. Decisión: así el puntaje habla del cambio.
4. **Canal lateral.** `Veredicto` vive fuera de la tabla de archivos. La muestra y la duración de la base viajan por un `thread_local` de `mutacion.rs`, como ya hace `con_archivo_acotado`. Decisión: no cambiar `Veredicto`. Si el planner quiere el dato en `Veredicto`, hace falta otra tarea.
5. **Casos ruidosos.** Un caso ruidoso sigue contando para las métricas. La fila solo suma `casos_ruidosos`. Decisión: sin descartar casos, como pide el brief.
