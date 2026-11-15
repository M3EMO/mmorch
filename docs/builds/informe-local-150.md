# Informe de la Tarea 150

Rama `laptop/tarea-150`, base 05a5364. Todo corrió con `ROOT_SIN_PODMAN=1`, sin podman, sin Colab y sin APIs.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (05a5364) | 3792 | 0 | 5 |
| Final (punto 4) | 3815 | 0 | 5 |

La suite corrió con `tools/test-seguro.sh`, dentro del hook de cada commit. El hook falló una vez en el primer intento del punto 1. Fallaron 11 tests de `tests/colab.rs`, por el guion falso sin `une-trozos.py`. No fue un test de tiempo. Corregí el falso y el segundo intento pasó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 5 | `verificar-versiones.sh` sin `RUSTUP_HOME` ni `CARGO_HOME` | `sin_rustup_home_el_guion_fija_las_rutas_de_rust`. Guarda: `rustup_home_del_entorno_se_conserva` | 5d75917 |
| 1 | Rutas remotas relativas y subida entera | `ciclo_completo_sube_y_baja_con_rutas_remotas_absolutas`, `paquete_chico_sube_un_solo_trozo_a_root_paq_00`, `paquete_de_100_mib_sube_tres_trozos_en_orden`, `guion_preparar_con_sha_malo_frena_sin_extraer`, `preparacion_con_falla_solo_en_stdout_la_muestra`, `colab_toda_ruta_remota_es_absoluta`, `colab_subir_en_trozos_*`, `colab_detalle_*`. Guarda: `trozo_que_falla_frena_la_subida_y_nombra_el_trozo_sin_token` | 50e8230 |
| 2 | Un SHA distinto decía "no llegó" | `vuelta_con_sha_del_cierre_distinto_no_se_importa`. Guardas: `falla_de_subida_termina_con_paquete_de_vuelta_no_llego`, `vuelta_que_no_baja_termina_con_paquete_de_vuelta_no_llego` | 0d67159 |
| 3 | Dos entradas para empaquetar | Solo guarda: `colab_empaquetar_sin_banderas_pasa_4_y_42_desde_la_cli` | 2245a90 |
| 4 | Eficiencia rechazada en Colab | `acepta_eficiencia_sin_juez`. Guarda: `rechaza_diseno_con_su_motivo` | ce36295 |

El punto 1 incluye lo que pidió la nota del orquestador. Todo error de una llamada a la CLI de Colab suma su stdout, sin el valor de `colab-runtime-proxy-token`. Lo hace `SalidaColab::detalle()`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- No probé nada en Colab real. Las rutas `/content/root-paq.NN` y la unión en el runtime quedan para la prueba de la 141.
- La guarda del punto 3 no llega a `colab_empaquetar` real, porque esa ruta llama a podman y a git. Prueba que los valores de la CLI llegan al armador. El tipo fija que el armador es `empaquetar_cli_con`.
- No hay test del plazo vencido en medio de la subida de un trozo.
- Ningún test usa podman. No escribí tests con `saltar_sin_podman`.

## 4. Lo que se borró

- `empaquetar_cli`. Su único llamador era `src/lib.rs:640`, por codegraph y por `grep -w`. Los informes de las Tareas 145 y 146 la nombran en texto, sin llamarla.
- El `if` de `validar_trabajos` que rechazaba Eficiencia.
- El cierre `armar` de `colab_empaquetar` en `src/lib.rs`.

## 5. Las rutas remotas

| Llamada | Antes | Después |
|---|---|---|
| `upload` del paquete en `colab_calibra.rs` | `paquete.tar.gz` | `/content/root-paq.NN`, unidos en `/content/paquete.tar.gz` |
| `download` de la vuelta | `/content/vuelta.tar.gz` | igual |
| `upload` del paquete en `colab.rs` | `root-paquete.tar.gz` | `/content/root-paq.NN`, unidos en `/content/root-paquete.tar.gz` |
| `download` de junit en `colab.rs` | `root-paquete/<nombre>` | `/content/root-paquete/<nombre>` |
| `upload` del binario en `colab.rs` | `root-paquete/target/debug/<nombre>` | `/content/root-paquete/target/debug/<nombre>`. Sobre 40 MiB va en trozos |
| `upload` del `cargo` mínimo | `root-paquete/bin/cargo` | `/content/root-paquete/bin/cargo` |

Los guiones de `colab.rs` (`GUION_PREPARAR`, `GUION_CORRER`, `GUION_ESTADO` y el de prueba del binario) usan ahora rutas absolutas. `PAQUETE_REMOTO` y `CARPETA_REMOTA` también.

## 6. Tests que cambiaron de valor

- `colab_rust_hash_malo_no_corre_rustup_y_usa_el_binario` (`colab.rs`): los literales de `upload` llevan ruta absoluta.
- Varios falsos de `colab.rs` responden `ROOT_UNIDO=ok` a `une-trozos.py`.
- `vuelta_con_sha_del_cierre_distinto_no_se_importa` y `sha_de_la_vuelta_distinto_no_se_abre`: leen el texto nuevo del SHA distinto.
- `rechaza_eficiencia_y_diseno_con_su_motivo` pasó a `rechaza_diseno_con_su_motivo`. La mitad de Eficiencia se reemplazó por `acepta_eficiencia_sin_juez`.

Tests de otros archivos:
- `tests/colab_preparar.rs`: dos tests nuevos del punto 5 y dos helpers.
- `tests/colab.rs`: los dos guiones falsos de la CLI responden `*/une-trozos.py`. Es el literal nuevo del guion de unión.

## 7. Llamadores revisados

- `empaquetar_cli`: codegraph da `colab_empaquetar` (`src/lib.rs:626`). `grep -w` da `src/lib.rs:640`. Ambos están migrados.
- `script_preparar(n, sha)`: su único llamador es `correr_runtime`.
- `PAQUETE_REMOTO_IDA`: lo usa `script_preparar`.
- `PAQUETE_REMOTO` y `CARPETA_REMOTA` de `colab.rs`: revisé cada uso con `grep -n`. Los guiones y los `exec` ven la ruta absoluta.
- `DIMENSIONES_COLAB`: la leen `validar_trabajos` y sus tests.

## 8. Preguntas para el planner

1. **`DIMENSIONES_COLAB` suma Eficiencia.** El brief pedía solo quitar el rechazo. Sin la suma, el filtro rechazaba Eficiencia con "dimensión no calibrable en Colab". La sumé.
2. **El sufijo "no llegó" se decide por el archivo.** Va solo si `vuelta.tar.gz` no bajó. Una vuelta que bajó y falla otra verificación sale sin sufijo. ¿Basta?
3. **`correr_runtime` lee el paquete entero para su SHA-256.** Son 182 MB en memoria, una vez. Los trozos sí se leen de a uno. ¿Se pasa a lectura por bloques?
4. **La línea de podman en `colab_calibra.rs` conserva `stderr.trim()`.** No es una llamada a la CLI de Colab.
5. **El SHA-256 no se compara en la subida de los binarios chicos.** Un binario de 40 MiB o menos sube directo, sin verificar SHA.
