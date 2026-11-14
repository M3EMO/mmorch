# Informe de la Tarea 149

Rama `laptop/tarea-149`, base 05a5364. Todo con `ROOT_SIN_PODMAN=1`. Solo cambió `src/oracle/eficiencia.rs`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (05a5364) | 3790 | 0 | 5 |
| Final (138211a) | 3803 | 0 | 5 |

Son los totales de todos los binarios de `tools/test-seguro.sh`. El hook de commit no falló por tiempo en ningún commit: no hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | No existía el guion `root-medir.py` | `guion_cpu_imprime_cpu_us_mayor_que_cero`, `guion_rss_deja_una_linea_de_100_mib_o_mas`, `guion_sale_con_el_codigo_del_comando` | 7b4bb8e |
| 2 | `memoria_pico` leía `memory.peak`, el pico del runtime entero | `cmd_memoria_fija_el_runner_y_no_lee_memory_peak`, `cmd_tests_corre_solo_los_tests_sin_compilar`, `sin_medida_de_memoria_memoria_no_tiene_valor_y_tiempo_si`, `con_referencia_serie_viva_fallida_deja_la_medida_sin_valor_con_su_motivo`, `medir_corre_la_memoria_de_cada_lado_despues_de_los_tests`, `medir_escribe_el_guion_en_cada_lado` | f0e1bfc |
| 3 | La carga de `/proc/loadavg` contaba los procesos de Root | `corrida_con_la_cpu_de_la_maquina_igual_a_la_propia_no_queda_cargada`, `seis_nucleos_ajenos_de_ocho_dejan_los_tiempos_sin_valor`, `con_dos_nucleos_1_5_ajenos_queda_cargada`, `corrida_sin_root_cpu_us_deja_los_tiempos_sin_valor`, `las_corridas_medidas_van_bajo_el_modo_cpu`, `comillas_conserva_las_comillas_simples`, `leer_cpu_ocupada_suma_solo_los_campos_ocupados`, `ruta_de_carga_obedece_root_proc_stat`, `una_sola_corrida_cargada_basta`, `el_aviso_lleva_la_mayor_carga_ajena_aunque_no_este_cargado` | 138211a |

Guardas verdes sin cambio: los tests de comparación por medianas y cuartiles; `carga_alta_no_quita_memoria_ni_tamano`; `base_cargada_deja_la_calibracion_sin_valor_y_el_caso_hereda_el_motivo` (reescrito sobre la fuente nueva).

## 3. Qué falló o quedó incompleto

- **Sin probar con podman.** Nadie corrió nextest con el runner dentro del contenedor ni en Colab. Lo ve el orquestador.
- **Aviso de carga ajena.** Cada caso suma `eficiencia: mayor carga ajena de una corrida: X núcleos de N` a `avisos` de `medir`. No se verificó que llegue a la fila del veredicto sin cambio de esquema. Hay que revisarlo.
- **Memo de sondas (punto 3.12).** Ninguna corrida medida pasa por el memo. `Alcance::Deterministas` excluye la Eficiencia, y `toma` no reconoce los comandos de Eficiencia ni el comando envuelto en `python3 ... cpu '...'`. No se agregó un test.
- **Borrado en caso de error.** Si `escribir_guion` falla en una carpeta que `medir` creó, la carpeta queda sin borrar. Es un caso raro. No se corrigió.
- **Lista de la fase `list`.** Nextest también llama al runner al listar tests (`--list`). Cada llamada agrega una línea chica a `root-rss.lista`. No altera el máximo, y la lista se borra al inicio de cada corrida de memoria.
- **Reloj en los tests.** Los tests de carga usan un reloj de 20 ms con `sleep`. Los de "cargada" necesitan que solo una corrida dure menos de 30 ms. Los de "no cargada" dan exactamente 0 o quedan lejos del umbral.

## 4. Lo que se borró

| Símbolo | Llamadores antes de borrar (codegraph y `grep -w`) |
|---|---|
| `con_control_de_carga` | `medir_los_dos_lados`, `medir_contra_referencia` |
| `leer_carga` | `con_control_de_carga` y un test |
| `motivo_de_carga` | `con_control_de_carga` |
| `struct Carga` | esas dos funciones |
| `ROOT_LOADAVG` y `/proc/loadavg` | `ruta_de_carga` y un test |
| `/sys/fs/cgroup/memory.peak` | `cmd_tests` |

Fuera de `eficiencia.rs` no había llamadores. `docs/builds/informe-local-144.md` y `147.md` citan `/proc/loadavg` como historia. No se tocaron.

## 5. El runner de nextest

Fuente: https://nexte.st/docs/features/target-runners/ (leída el 2026-11-14).

> "The environment variable `CARGO_TARGET_<triple>_RUNNER`, if it matches the target platform, takes highest precedence."

> "For tests that run on the host platform, nextest uses the target runner defined for the host."

La página no dice de forma literal qué pasa sin `--target`. Sin `--target`, la plataforma target es el host. El triple `x86_64-unknown-linux-gnu` coincide con ella. Es una deducción, no una cita. La suite con podman debe confirmarla: si la lista `root-rss.lista` queda vacía, `memoria_pico` queda sin valor con "sin medida de memoria por proceso", y la falla se ve.

La página dice también que el runner corre en la fase de listado, así que no debe escribir en stdout. El modo `rss` no escribe.

## 6. Tests que cambiaron de valor

En `eficiencia.rs`:
- `cmd_tests_corre_solo_los_tests_sin_compilar`: ahora exige que no haya `memory.peak`, `ROOT_MEM` ni `RUNNER`.
- `sin_memory_peak_memoria_no_tiene_valor_y_tiempo_si`: pasa a `sin_medida_de_memoria_...`, con el motivo nuevo.
- `con_referencia_serie_viva_fallida_...`: motivo nuevo.
- `con_referencia_el_inicial_no_pide_metadata_ni_lista` y `con_referencia_la_razon_usa_la_serie_guardada`: suman las corridas de `memoria` entre `tests` y `build`.
- `ruta_de_carga_obedece_root_loadavg` pasa a `ruta_de_carga_obedece_root_proc_stat`.
- Los tests de carga usan un `proc/stat` falso. Comparan el motivo por prefijo y sufijo, porque el valor depende del reloj.
- `lectura_ilegible_...`: el caso sin núcleos usa una máquina con CPU propia, para aislar su motivo.
- Borrados: `carga_de_exactamente_la_mitad_de_los_nucleos_no_es_carga` (el reloj real tiene jitter) y `la_lectura_despues_de_las_corridas_tambien_cuenta` (la reemplaza `una_sola_corrida_cargada_basta`).

Tests de otros archivos que edité: ninguno.

## 7. Llamadores revisados

| Símbolo | codegraph | `grep -w` |
|---|---|---|
| `cmd_tests` (ya no imprime `ROOT_MEM`) | `correr_tests` | solo `eficiencia.rs` |
| `correr_tests` (pierde `memoria`, gana `Vigilancia`) | `medir_tests` | solo `eficiencia.rs` |
| `medir_tests`, `medir_compilacion` (firma nueva) | `medir_contra_referencia`, `medir_los_dos_lados` | solo `eficiencia.rs` |
| `medir_los_dos_lados` (suma `avisos`) | `medir_lados` | solo `eficiencia.rs` |
| `correr_build` | `medir_compilacion` | `eficiencia.rs` y `entrega.rs`; el de `entrega.rs` es otra función con el mismo nombre |
| `leer_carga`, `motivo_de_carga`, `con_control_de_carga`, `ROOT_LOADAVG` | sin llamadores tras el borrado | vacío |

## 8. Preguntas para el planner

1. **Orden de los motivos de una corrida.** El organizador decidió: primero el error de lectura de `/proc/stat`, luego "sin medida de CPU propia", luego los núcleos. Solo se guarda el primer motivo de la medición.
2. **Sin núcleos leídos, sin aviso.** El organizador decidió que `aviso()` da `None` si `available_parallelism` falla. El motivo del caso sí lleva ese error.
3. **Las corridas de memoria no pasan por el control de carga.** El brief lista solo las de tiempo y las compilaciones. El organizador siguió el brief: la memoria por proceso no depende de la carga.
4. **Falta `steal` en kernels viejos.** Una línea `cpu` con menos de 8 campos da error de lectura y los tiempos quedan sin valor. El organizador prefirió eso a suponer `steal` cero.
