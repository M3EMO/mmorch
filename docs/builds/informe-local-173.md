# Informe de la Tarea 173

Base: `laptop/junta-171-172` (3ae7370). Rama: `laptop/tarea-173`. Sin push. No usé `codegraph`; los llamadores salen de `grep -w`.

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (3ae7370) | 4184 | 0 | 5 |
| Final | 4219 | 0 | 5 |

Los tests que necesitan podman no corrieron. Ningún test nuevo usa podman. Los tests reales de `bwrap` se saltan solos, porque este runtime no tiene `bwrap` usable.
Los cinco commits de código y este informe pasaron el hook completo. No hubo reintentos por tests de tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1. Oráculo en el paquete de `corre` | El paquete no traía `herramientas/`, `versiones.env`, `entorno.env` ni el id de la imagen. El runtime no fijaba las dos variables de bwrap | `corre_paquete_trae_herramientas_versiones_y_entorno_del_oraculo`, `corre_manifiesto_trae_el_id_de_la_imagen_del_oraculo`, `empaquetar_corre_con_sha_invalido_da_uso_antes_de_armar`, `colab_empaquetar_corre_sin_rustup_sha256_falla`, `corre_runtime_pasa_al_r00t_el_entorno_y_el_id_de_la_imagen`, `corre_runtime_sin_imagen_id_en_el_manifiesto_falla_sin_correr`. Guarda: `paquete_de_calibracion_guarda_shas_del_oraculo` | a2bb3e3 |
| 2. Token por archivo | El guion pedía `ROOT_TOKEN_CUPO` | `corre_runtime_mueve_el_token_del_archivo_y_borra_el_original`, `..._token_nuevo_es_0600_con_el_contenido_del_archivo`, `..._rechaza_root_token_cupo_sin_correr_ni_mostrar_el_valor`, `..._sin_archivo_o_vacio_no_corre_nada`, `..._si_el_destino_falla_conserva_el_original`. Guardas: `corre_runtime_borra_el_token_al_terminar`, `corre_runtime_no_filtra_clave_ni_token`, `..._rechaza_el_mismo_archivo_como_origen_y_destino` | 3c82102 |
| 3. Sin `.credentials.json` con token | Con token, el builder exigía y montaba la credencial | `corre_bwrap_con_token_lanza_sin_credencial_en_el_host`, `..._no_copia_ni_monta_la_credencial`, `..._con_token_de_modo_abierto_frena_sin_mostrar_contenido`, `corre_bwrap_token_vacio_frena_antes_de_copiar_la_credencial`. Guardas: `corre_bwrap_sin_token_sin_credencial_frena_con_el_mensaje_de_siempre`, `corre_bwrap_sin_token_copia_la_credencial_y_la_monta` | 438a3ef |
| 4. `claude` y `node` bajo `/usr` | No se revisaba dónde caían | `preparar_corre_npm_instala_claude_con_prefix_usr_local`, `..._root_npm_por_omision_es_usr_bin_npm`, `..._frena_si_claude_fuera_de_usr`, `..._frena_si_node_fuera_de_usr`, `..._revisa_claude_por_ruta_real_del_enlace`, `..._frena_si_claude_no_se_encuentra`. Guardas: los 3 `preparar_corre_*` previos | 2a2c474 |
| 5. IPv6 y logs de red | IPv6 abierto; logs de slirp4netns a `/dev/null` | `apaga_ipv6_y_pone_drop_en_ip6tables`, `sin_ip6tables_frena_antes_de_crear_el_espacio`, `log_de_slirp_va_al_archivo_con_modo_0600`. Guardas: los 5 tests previos de `tests/red_builder.rs` | d60b6b1 |
| 6. Importación sin duplicados | Un reintento agregaba la fila otra vez | `importar_corre_reintento_tras_fallar_la_copia_no_duplica_la_fila`, `..._transcripts_ya_copiados_e_iguales_se_aceptan`, `..._copia_a_medias_previa_se_borra_y_no_queda_parcial`, `..._mismo_ts_y_corrida_con_tipo_distinto_entran_los_dos`. Guardas: `..._fallo_despues_de_los_eventos_el_reintento_no_los_duplica`, `..._transcripts_distintos_frenan_antes_de_escribir`, `..._evento_con_misma_corrida_ts_y_tipo_no_entra_dos_veces`, los 11 de la 172 | acd4279 |

Notas de honestidad sobre el rojo:
- Punto 1: los subagentes vieron el rojo con la forma nueva de las firmas y sin lógica. La guarda de los SHA de calibración pasó antes y después del refactor, como debe.
- Punto 2: `..._rechaza_el_mismo_archivo_como_origen_y_destino` pasó desde el inicio, por otro motivo (faltaba `ROOT_TOKEN_CUPO`). Es una guarda débil.
- Punto 6: el fallo "después de los eventos" usa `runs.jsonl` como carpeta. `colab-corre.json` como carpeta falla al leerlo, antes de escribir nada.

## 3. Qué falló o quedó incompleto

- **Nada se probó en Colab.** `preparar-corre.sh` con apt y npm reales, bwrap real, IPv6 real y la prueba de aislamiento los corre el orquestador.
- **Sin probar con `ip6tables` real:** si el kernel de Colab no trae el módulo, `ip6tables -P` fallaría y el guion terminaría con error.
- **La revisión de `claude` bajo `/usr/` usa `readlink -f`.** Con el prefijo de producción `/usr/` no se probó contra el `npm` real.
- **Punto 1:** el paquete de `corre` ahora llama a podman para leer la imagen `localhost/root-oraculo-rust:4`, igual que la calibración. En la laptop, esa imagen debe existir. No la probé.
- El `touch` + `chmod 0600` del log de slirp deja una ventana mínima con modo por defecto, sobre un archivo vacío.

## 4. El entorno del runtime

El `r00t` hijo de `corre-runtime.py` recibe una copia del entorno del guion, sin `DEEPSEEK_API_KEY` ni `ROOT_TOKEN_CUPO` heredadas, más:

| Variable | Valor | De dónde sale |
|---|---|---|
| `ROOT_HOME` | `ROOT_CORRE_HOME`, defecto `/root/root-home` | el guion |
| `ROOT_CONTENEDOR_MODO` | `bwrap` | fijo |
| `ROOT_RED_BUILDER` | `root-builder` | fijo |
| `ROOT_USUARIO_BUILDER` | `builder` | fijo |
| `ROOT_TOKEN_BUILDER` | ruta `ROOT_CORRE_TOKEN`, defecto `/run/root-token` | el guion movió ahí el archivo de `ROOT_TOKEN_ARCHIVO` (defecto `/content/root-token`) |
| `DEEPSEEK_API_KEY` | la clave | el guion la saca de `os.environ` y la pasa solo al hijo |
| `ROOT_BWRAP_ENTORNO` | `<carpeta del paquete>/entorno.env` | nuevo, punto 1 |
| `ROOT_BWRAP_IMAGEN_ID` | `imagen_id` del manifiesto | nuevo, punto 1 |

`ROOT_TOKEN_CUPO` ya no existe: si el entorno la trae, el guion sale con 2 y nombra la variable, nunca su valor.
Nota: `ROOT_BWRAP_ENTORNO` es una ruta absoluta; la calibración usa la relativa `entorno.env`. La carpeta del paquete es el directorio de trabajo del runtime, así que ambas apuntan al mismo archivo.

## 5. Tests que cambiaron de valor

Ninguna aserción cambió. Cambiaron helpers y entradas:

- `src/cli.rs`: `colab_empaquetar_corre_parsea_tareas_config_y_pasada`, `..._con_trabajos_falla`, `..._sin_pasada_falla` suman `--rustup-sha256` (ahora es obligatorio con `--corre`). Es un test de otro archivo que lee una firma que cambia.
- `src/colab_corre.rs`: `pedido()` suma `rustup_sha256`; `empaqueta()` pasa un doble de `Imagen`; `Corrida::nueva` suma `imagen_id` y el archivo de token; `Corrida::correr` pasó a `correr_con` y fija `ROOT_TOKEN_ARCHIVO`; `Prep::nuevo` y `Prep::correr` suman `node` falso, `npm` que anota sus argumentos y las variables `ROOT_NPM`, `ROOT_PREFIJO_USR`, `ROOT_PATH_BUILDER`.
- `src/corre/bwrap.rs`: dos tests dejan el token en 0600 con el helper `escribir_token`, porque el chequeo previo ahora exige ese modo. Los llamadores de `args_bwrap_builder` y `armar_comando` pasan `Some(credencial)`.
- `tests/red_builder.rs`: el `slirp4netns` falso escribe una línea; `escenario()` suma `sysctl` e `ip6tables` falsos; `correr()` se partió en `path_del_test` y `correr_con_path`.

## 6. Llamadores revisados

Con `grep -w`:

| Firma cambiada | Llamadores |
|---|---|
| `empaquetar_corre` (suma `oraculo`) | `empaquetar_corre_cli` y el helper `empaqueta` de los tests, ambos en `src/colab_corre.rs` |
| `PedidoEmpaquetarCorre` (suma `rustup_sha256`) | `src/lib.rs` y `pedido()` de los tests |
| `armar_oraculo_runtime` (nueva, pública) | `armar_y_empaquetar` en `src/colab_calibra.rs` y `armar` en `src/colab_corre.rs` |
| `binario_podman` (ahora pública) | `src/colab_calibra.rs` y `empaquetar_corre_cli` |
| `leer_manifiesto` de Python (devuelve 3 valores) | solo `principal` de `corre-runtime.py` |
| `args_bwrap_builder` y `armar_comando` (`Option<&Path>`) | `lanzar` y 12 usos en tests, todos de `src/corre/bwrap.rs` |
| `ClaveEvento` y `pendientes_de` (suma el tipo) | `agregar_eventos` y los tests de `src/colab_corre.rs` |

## 7. Preguntas para el planner

1. **`src/lib.rs` quedó fuera de la tabla, y lo toqué.** `PedidoEmpaquetarCorre` suma `rustup_sha256`, y `lib.rs` arma ese pedido. Sin 7 líneas en `lib.rs` el crate no compila. **Decisión:** hice el cambio mínimo (extraer `rustup_sha256` y pasarlo). Si el planner no lo acepta, se revierte `lib.rs` y el pedido lee el SHA por otro camino. El `git diff --stat` del orquestador mostrará este archivo.
2. **Los tests de `preparar-corre.sh` viven en `src/colab_corre.rs`,** que el punto 4 no lista. **Decisión:** agregué los tests ahí, porque no hay otro lugar.
3. **`--rustup-sha256` ahora es siempre obligatorio** en `colab empaquetar`, con y sin `--corre`. **Decisión:** `required = true`, y salió de los conflictos de `--corre`.
4. **Dos eventos con la misma corrida, el mismo `ts` y el mismo tipo cuentan como uno,** aunque traigan costos distintos. Es lo que pide el punto 6.4. Con el `ts` de coma flotante y eventos de gasto muy seguidos, una colisión pierde gasto. **Decisión:** acepté la clave del brief, que además conserva `reserva`.
5. **`ROOT_TOKEN_ARCHIVO` igual a `ROOT_CORRE_TOKEN` da error** (el original se borraría antes de correr). **Decisión:** salida con 2. El brief no lo pedía.
6. **Un `transcripts/<id>` ya existente con contenido idéntico se acepta** en la importación; si difiere, sigue el error de la 172. Sirve para completar un reintento. **Decisión:** comparar archivo por archivo y byte por byte.
7. **El token con modo 0644 frena el lanzamiento** y el mensaje nombra el modo, no el contenido. En Colab, `corre-runtime.py` lo crea en 0600, así que no choca.
