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
