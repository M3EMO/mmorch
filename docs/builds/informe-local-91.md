# Informe de la Tarea 91

Rama `laptop/tarea-91`, base 599dcd8. Todo corrió con `ROOT_SIN_PODMAN=1`.
Haiku construyó cada parte. El organizador revisó cada diff antes del commit.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (599dcd8) | 3015 | 0 | 4 |
| Final | 3047 | 0 | 4 |

Los tests con `saltar_sin_podman!` no corrieron contra podman real. Compilan, pero su comportamiento con podman no está verificado.
El orquestador debe correrlos antes de mezclar.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1, MEDIUM 12 | El repo falsificaba el paso que falló y el texto de red. | `entrega_step_impreso_por_el_repo_no_decide_el_paso`, `entrega_red_con_red_que_falla_igual_en_el_mismo_paso_es_hallazgo_sin_aviso`, `entrega_sin_linea_final_de_podman_da_sin_veredicto_con_pasos_visibles`, `entrega_red_no_configurada_solo_mira_la_linea_final_del_stderr`, `entrega_con_red_slirp_impreso_por_el_repo_no_es_error_de_podman`. Guardas: imagen base ausente (3), slirp4netns, y la de podman con un STEP falso. | 9d76c7a |
| 3, junit | Faltaba fijar que Root lee el junit con el contenedor muerto. | Ninguno: ya se cumplía. Guardas `oraculo_el_junit_se_lee_con_el_contenedor_terminado` y `oraculo_sin_junit_al_terminar_el_contenedor_es_infra`. | b1fb721 |
| 2, MEDIUM 15 | Un proceso del producto escribía `0` en los `.rc` de `/salida`. | 26 tests `eval_*` y de entrega con sesión falsa, 6 de `contenedor.rs` y 11 de `Sesion`. Guarda con podman: `entrega_podman_proceso_de_fondo_que_escribe_rc_no_pasa_un_escenario_que_falla`. | 10a58b7 |

Dónde se espera el fin del contenedor (punto 3): `contenedor::ejecutar` espera a podman con `esperar` y a los lectores con `recoger`.
Recién después `juzgar_junit` lee el junit.

El reintento de commit por un test de tiempo no hizo falta.
El primer commit del punto 1 falló en el hook por dos tests de `mod.rs`. Los arreglé y repetí el commit.

## 3. Qué falló o quedó incompleto

- Ningún test con podman real corrió. Incluye la guarda del proceso de fondo y `guarda_escenario_sin_capacidades_ni_procesos_de_mas`.
- `terminar()` del hijo de `correr` mata el cliente `podman exec`, no un grupo de procesos. Lo que corre dentro muere con `podman rm -f`.
- Quedó `/tmp/eval2b`, un directorio de trabajo de un subagente. Ningún código ni test lo crea. Un hook frenó su borrado.

## 4. Tests que cambiaron de valor

- Punto 1. `entrega_paso_guardado_clasifica_si_la_cola_perdio_el_step` y `entrega_si_la_cola_trae_un_step_manda_la_cola_y_no_paso` ya no deciden por `paso` ni por un STEP del medio. Llevan la línea final de podman.
- Punto 1. `entrega_red_sin_red_en_el_host_queda_pendiente_con_aviso` se borró. Con red, un fallo ahora es rojo. Lo cubre `entrega_red_con_red_que_falla_igual_en_el_mismo_paso_es_hallazgo_sin_aviso`.
- Punto 1. Seis tests usan `salida_red_no_configurada()` en el build con red, para conservar su intención de pendiente. Dos están en `mod.rs`.
- Punto 1. `salida_sin_red()` trae ahora la línea final de podman.
- Punto 2. Los tests que fijaban el texto del script (`.rc`, `tail -n 40`, marcador) se borraron o se reemplazaron por tests de los argumentos de cada paso.
- Punto 2. Los tests con `Falso::con(.rc)` pasaron a la sesión falsa. Mantienen sus estados.
- Punto 2. `entrega_escenario_fallido_deja_requisito_en_parcial` y `entrega_dockerfile_sin_user_da_un_solo_hallazgo_entre_configs_y_entrega` se movieron de `tests/entrega.rs` a `entrega.rs`.
- Punto 2. `entrega_podman_log_de_correr_queda_en_la_cola_de_40_lineas` pasó a `entrega_podman_correr_que_muere_tras_mucha_salida_cita_su_ultima_linea`. El log ya no es un archivo.
- Punto 2. Dos tests de `tests/entrega.rs` cambiaron el comando de `test -f /hola.txt` a `true`. Un `.rc` falso ya no lo marca verde.

## 5. Archivos de `/salida` que quedan

- El contenedor de evaluación monta `/salida` como carpeta temporal bajo `home/tmp`.
- Solo escribe `/salida/work`, la copia de `/repo`, y lo que el producto deje ahí.
- Ningún código de Root lee archivos de `/salida` en la entrega. No hay `.rc`, `.log`, `.fin` ni marcador.
- El junit de las suites se lee de `/salida` con el contenedor ya muerto. Se cruza con el código de `podman run`.

## 6. Llamadores revisados

CodeGraph no estaba inicializado en este worktree. Usé `grep -w` en todo `src/` y `tests/`.

- `clasificar_fallo` (firma nueva con `con_red`): `construir_con` (2 usos) y los tests.
- `texto_paso_fallido`, `AVISO_RED_TAMPOCO`: borrados, sin otros usos.
- `red_no_configurada`: `construir_con` y un test. Ahora mira solo la última línea del stderr.
- `ContenedorEscenario`: reemplazado por `Sesion`. Sin usos fuera de `entrega.rs`.
- `construir_script` y `fallo_de_infraestructura`: borrados. `revisar_salida`, `ARCHIVO_ARRANQUE`, `ARCHIVO_PRELUDIO`, `LOG_CORRER_COMPLETO`, `comillas_simples`, `LINEAS_LOG` y `LINEAS_ESCENARIO` también. Sin usos.
- `construir_script_con`: queda como `#[cfg(test)]` y devuelve texto descriptivo de los pasos. Solo lo llama el test de `memo.rs:431`, que no pude editar.
- `entregar`, `sonda_entrega`: pierden `ej`. Llamadores: `tests/entrega.rs`.
- `entregar_con`: pierde `ej`. Llamadores: `mod.rs` (una línea) y tests de `entrega.rs`.
- `FabricaEscenarios`: gana `iniciar_evaluacion` con implementación por defecto. Los fakes de `calibra.rs` no cambiaron.

Archivos fuera de la tabla del brief: `src/oracle/mod.rs` (call site de `entregar_con` y dos tests del punto 1) y `tests/entrega.rs` (API nueva).
No cambió ningún esquema serde. `docs/interfaces.md` no necesita regeneración.

## 7. Preguntas para el planner

1. Un escenario con pendiente de red en el contenedor de evaluación. Decisión: igual que la imagen, sin entrada en el mapa y con un `Pendiente`.
2. Plazo de cada paso de la entrega. Decisión: `PLAZO_PASO_ENTREGA` de 15 minutos. Confirmar.
3. RustSec en el contenedor de evaluación. Decisión: no se monta, como pedía el diseño. Antes el ejecutor viejo lo montaba.
4. `/oraculo` en el contenedor de evaluación. Antes no montaba nada. Ahora monta solo `escenarios/` en `/oraculo/escenarios`, como la imagen. `oculto/` y `tarea.toml` quedan afuera.
5. `iniciar_evaluacion` no recibe `Entorno`. El entorno va en cada paso, porque el preludio y la copia corren sin activación. Es un desvío menor del diseño.
6. Preludio de Rust: solo el código 125 es infraestructura. El resto lo aísla el `true` final, como antes.
7. `red_no_configurada` ahora mira solo la última línea del stderr. El diseño no lo pedía de forma explícita. Lo hice por la regla 3.
8. Imagen base ausente sin línea `building at STEP` (error `creating build container`). Decisión: da pendiente con la ubicación del `FROM` si es única. El brief no fijaba esa forma.
9. El riesgo aceptado de D2 sigue abierto: un proceso puede alternar rojo y verde entre tests.

# Tarea 91b: corrección de la Tarea 91

Rama `laptop/tarea-91`, base ea76273. Haiku construyó cada parte. El organizador revisó cada diff antes de su commit.
El brief pedía `informe-local-91b.md`. La orden del usuario pidió agregar esta sección a este archivo.

## 1. Resultado de la suite (91b)

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base sin podman (ea76273) | 3047 | 0 | 4 |
| Final sin podman (3e393e0) | 3061 | 0 | 4 |
| `tests/entrega.rs` con podman, antes (ea76273) | 34 | 14 | 0 |
| `tests/entrega.rs` con podman, final | 48 | 0 | 0 |
| `--lib entrega` con podman, final | 151 | 0 | 0 |

- La base con podman viene del informe del orquestador: 14 fallas de 48 tests de `tests/entrega.rs`.
- La suite completa con podman la corre el orquestador.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales.

## 2. Causa de la clase A

- El texto real de podman: `Error: i: image not known`. El contenedor de evaluación no arrancaba ni con `sleep` ni con `tail`.
- La causa no era un montaje. Era la imagen. `tests/entrega.rs` escribía `imagen = "i"` en el `tarea.toml`.
- Antes del punto 2 de la Tarea 91, los tests pasaban `ALPINE` a un ejecutor. Desde entonces, el contenedor usa `tarea.imagen`.
- El arreglo: los tres `tarea.toml` de prueba usan `docker.io/library/alpine:3.20`.
- Además, `Sesion::iniciar` ahora conserva el stderr de cada `run` fallido, y `iniciar_evaluacion` llama a `validar_rutas` antes del `run`.
- Con eso, los 11 tests de la clase A quedaron verdes.

## 3. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El error de `run` se perdía, y no se validaban las rutas | `sesion_iniciar_incluye_el_stderr_de_cada_run_fallido`, `iniciar_evaluacion_valida_rutas_antes_de_podman`, y con podman los 11 de la clase A | a19338d |
| 2 | HIGH-2 (125 del producto) y MEDIUM-1 (`PATH`) | `sesion_paso_args_pone_el_path_del_sistema_despues_de_la_activacion`, `sesion_correr_escenario_con_125_del_producto_es_fallo`, `eval_instalar_con_125_del_producto_no_instala`, `sesion_paso_args_con_timeout_lo_pone_dentro_del_envoltorio` | 1d85458 |
| 3 | Clase B y HIGH-3 (la red por texto) | repetición sin mirar el texto, sonda en rojo con su motivo, `RUN echo "setup network"; false` con sonda verde, 7 de ubicación del paso | b3c358b |
| 4 | HIGH-1 (`pide_red` en escenarios) | `sesion_correr_escenario_con_texto_de_red_y_sin_red_es_fallo`, `sesion_correr_escenario_con_red_queda_pendiente_sin_correrlo`, `eval_escenario_con_red_queda_pendiente_sin_correrlo` | 3e393e0 |

Guardas sin rojo previo:
- Punto 2: `envoltorio_deja_pasar_126_y_127_como_codigos_del_producto`, `eval_preludio_de_rust_con_codigo_125_es_infraestructura`.
- Punto 2, con podman: `guarda_podman_envoltorio_el_wget_del_sistema_gana_al_wget_falso_del_producto`.
- Punto 3: con la sonda verde el build con red que falla da rojo, y un `COPY` local da rojo sin repetir.
- Punto 4: `escenario_red_por_defecto_es_falso_y_con_red_true_es_verdadero`.

## 4. Qué falló o quedó incompleto

- El test con podman del `PATH` usa `wget`, no `curl`. Alpine no trae `curl`, y un `curl` falso no tapa nada. El motivo es el mismo.
- Un `PATH` con el sistema primero protege solo las herramientas que el sistema tiene. Una herramienta que solo aporta el producto, como `pytest` del venv, sigue saliendo del producto.
- La sonda corre en `tarea.imagen` con `--pull=never`. Con una imagen ausente o sin `sh`, da "sin red" y deja pendiente.
- La sonda se guarda una vez por proceso para todas las imágenes.
- El pendiente "la red no se pudo configurar en este host" ya no lleva la línea del `RUN`. El aviso lleva la última línea del stderr.
- `timeout` ahora corre dentro del envoltorio. La cita de un escenario que supera el plazo cambió. El código sigue siendo 124.
- No corrí la suite completa con podman, ni la validez, `corre`, `calibrar` ni la pasada.

## 5. Tests que cambiaron de valor

- Cuatro de `paso_args`: la forma de los argumentos ahora incluye el envoltorio, con el `timeout` adentro.
- `entrega_red_setup_network_con_red_queda_pendiente_con_aviso`, `entrega_red_pasta_fallida_con_red_queda_pendiente_con_aviso` y siete más de red en `entrega.rs` y `mod.rs`: la red mal configurada ahora la dice la sonda en rojo, no un texto.
- `oraculo_hallazgo_de_la_construccion_llega_al_veredicto_en_entrega`: ahora un `RUN false` también se repite con red, y falla con red.
- Siete de ubicación del paso (`clasifica_con_red`): miden el paso, no la regla de red.
- `tests/entrega.rs`, clase B: aceptan tres desenlaces. Con red pasa (severidad 1), rojo en la línea N, o pendiente con el motivo de la sonda.
- `sesion_correr_escenario_sin_red_queda_pendiente` pasó a `..._con_texto_de_red_y_sin_red_es_fallo`: un texto de red ya no decide.
- `eval_escenario_con_red_queda_pendiente_fuera_del_mapa` pasó a `..._sin_correrlo`: ahora usa `red = true`.
- `entrega_escenario_con_red_queda_pendiente_y_no_false` pasó a `entrega_escenario_que_declara_red_queda_pendiente_y_no_false`: el escenario declara `red = true`.
- Borrados porque solo probaban `pide_red`: `entrega_pide_red_no_confunde_un_servicio_caido_con_falta_de_red`, `entrega_pide_red_reconoce_los_errores_de_apk_y_wget`.
- `entrega_run_con_podman_real_que_imprime_step_falso_da_la_linea_del_run`: usa Alpine como imagen de la tarea, porque la sonda corre en ella.

## 6. Llamadores revisados (91b)

Codegraph y `grep -w` coinciden en todos.
- `Podman::red_disponible` (nuevo): solo `construir_con`. `PodmanReal` lo sobreescribe.
- `pide_red`, `PATRONES_RED`: borrados. Cero usos en `src/` y `tests/`.
- `validar_rutas`: ahora `pub(crate)`. Llamadores: `ejecutar` e `iniciar_evaluacion`.
- `paso_args`: llamadores `paso`, `lanzar` y sus tests. El nuevo `envoltorio_de` solo lo llama `paso_args`.
- `clasificar_fallo`: sin cambio de firma. Llamadores: `construir_con` y los tests.
- `Escenario` (campo `red`): literales de `entrega.rs`, `mod.rs` y `tarea.rs`. Las otras estructuras `Escenario` (`barrido.rs`, `mutacion.rs`, `tests/colab.rs`, `tests/corre.rs`) son distintas y no cambian.
- `docs/interfaces.md`: solo cambia la fila `[[escenario]]`. `Escenario` no figura en los esquemas, así que no hubo regeneración. `tests/interfaces.rs` pasa.

## 7. Preguntas para el planner

1. **`PATH` del sistema primero.** El brief lo pide. Un `python` o `pytest` del venv puede quedar tapado por el del sistema. Decisión: lo hice como pedía. Confirmar con un repo de Python real.
2. **La sonda usa `tarea.imagen`.** Decisión: sí, con `--pull=never` y `--http-proxy=false`. Una imagen ausente da "sin red".
3. **Cualquier `RUN` que falla se repite con red.** Incluso un `RUN false`. Decisión: sí, como pide el diseño. Cada repetición registra su Gate.
4. **`tarea.toml` existentes con escenarios que necesitan red.** Hoy dan fallo, no pendiente. Decisión: no los cambié. Hay que agregar `red = true` en cada tarea que los use.
5. **Se perdió la regla "connection refused no es red".** Ya no existe ningún patrón de red. Decisión: es el diseño.
6. **Cita del plazo.** Un escenario que supera 120 s ahora da código 124 y la cita es la última línea de su salida. Decisión: lo dejé así.

## Corrección 91c: el PATH del sistema no tapa el venv de Root

**Defecto.** El envoltorio de la 91b exportaba el PATH del sistema después de la activación. En `python:3.12-slim`, `python` resolvía en `/usr/local/bin` y `sys.prefix` valía `/usr/local`. Las dependencias del venv quedaban invisibles.

**Arreglo.** `envoltorio_de` arma el PATH en un solo lugar. El export del sistema va antes de la activación. La activación antepone `/tmp/venv-eval/bin` o `/tmp/pydeps/bin`. El orden queda: Root, sistema, resto. `ACTIVAR_RUST` no agrega `/tmp/cargo-home/bin` al PATH, así que no cambia.

**Tests.**
- Nuevo `guarda_podman_envoltorio_el_venv_de_root_gana_al_python_del_sistema`. Con el envoltorio viejo da `/usr/local` (rojo). Con el arreglo da `/tmp/venv-eval` y `/tmp/venv-eval/bin/python`.
- La guarda del `wget` falso sigue verde. El falso ahora vive en `/tmp/producto/bin`, un directorio que no es de Root y que va detrás del sistema.
- Los envoltorios esperados de los tests unitarios y el test de orden se actualizaron.

**Resultados con podman (sin `ROOT_SIN_PODMAN`).**
- `cargo test --lib entrega`: 152 pasan, 0 fallan.
- `cargo test --test entrega`: 48 pasan, 0 fallan.
- Las dos guardas con podman real pasan.
