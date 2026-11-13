# Informe de la Tarea 108

## 1. Resultado de la suite

- Base (d7dfcb0): no se midió aparte. El organizador no la corrió antes de empezar.
- Final (205682c), con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`: 2747 verdes y 0 fallas en la biblioteca.
- Los demás binarios de test dan 0 fallas. Hay 2 ignorados en un binario de integración.
- Los tests con podman quedan para el orquestador. No se corrió podman.
- El hook de commit no falló por tiempo. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El plazo vencido se reconocía por texto. | `error_de_plazo_lleva_el_plazo_y_el_nombre_del_contenedor`, `plazo_vencido_se_reconoce_por_variante_no_por_texto`, `plazo_de_calibracion_vencido_sin_junit_es_sin_veredicto_con_su_motivo`. Guarda: `plazo_vencido_con_su_mensaje_de_siempre_y_codigo_3`. | fc566f8 |
| 2 | Un plazo de Root abortaba la calibración. | `calibra_defecto_con_plazo_vencido_con_junit_va_a_reemplazo`, `calibra_limpio_con_plazo_vencido_con_junit_queda_sin_veredicto`, `calibra_dos_limpios_de_cinco_con_plazo_vencido_no_calibra`, `calibra_limpio_que_pasa_su_plazo_queda_sin_veredicto_y_no_aborta`. Guarda: `calibra_base_con_plazo_vencido_sigue_abortando`. | 205682c |
| 3 | `corre::barrido` tenía su propia regla de pid vivo. | `r00t_con_sha_vivo_no_se_barre`. Guardas: los tests de barrido de las Tareas 84, 85 y 87, con el margen de 300 s. | e3f8cca |

El punto 3 se construyó en la rama `laptop/tarea-108-p3` y se pasó con `cherry-pick`. El commit original es a887a88.

## 3. Qué falló o quedó incompleto

- `src/oracle/descarga.rs:741` solo trata `ErrorOraculo::Infra`. Un plazo vencido cae en `Desconocido`.
  El mensaje de la descarga cambia, aunque el código de salida sigue en 3.
  El archivo no está en la lista del brief. No se tocó. Ver pregunta 1.
- Ningún test de `ejecutar` con podman real prueba la variante. El cableado es una línea. La función pura `error_de_plazo` sí tiene test.
- El test de 2 de 5 limpios vencidos usa n=10. También lo corta `MIN_LIMPIOS`. No aísla el 20 %. Aislarlo pide n>=20.
- Solo Estructura tiene test de limpio vencido. Diseño y Arquitectura (con juez) no.
- No se corrieron la validez, `corre`, `calibrar` ni la pasada reales.

## 4. Tests que cambiaron de valor

- `calibra_limpio_que_pasa_su_plazo_es_error_y_no_se_mide` pasó a `..._queda_sin_veredicto_y_no_aborta`. Ahora el limpio queda sin veredicto y la calibración sigue.
- `vencimiento()` en `mod.rs` devuelve la variante nueva.
- `plazo_vencido_fuera_de_calibracion_...` y `plazo_vencido_con_junit_en_calibracion_...` pierden `_de_infra` en el nombre. Comprueban la variante.
- `corrida_con_pid_del_proceso_de_test_no_r00t_se_barre` pasó a `..._se_conserva`. `r00t_vivo` cuenta el pid propio como vivo. Su id usa los segundos de ahora, para no pasar el margen.

## 5. Llamadores revisados

- `PREFIJO_VENCIMIENTO`: borrada. Solo tenía el uso de `vence_plazo_sin_salida`. Grep y codegraph coinciden.
- `ErrorOraculo` (variante nueva): ningún `match` exhaustivo se rompió. `codigo_de_salida` la cubre con `_ => 3`.
  Revisados por grep: `descarga.rs:741` (ver pregunta 1) y `descarga/python.rs:567` (pasa la variante sin redactar; el nombre del contenedor no lleva secretos).
- `es_r00t` de `barrido.rs`: borrada. Codegraph y grep dan 0 llamadores. La `es_r00t` de `limpieza.rs` es otra función y sigue.
- `r00t_vivo`: ahora `pub(crate)`. Su único llamador nuevo es `motivo_de_muerte`.
- `evaluar_copia` y `medir_caso`: firmas sin cambio. Codegraph y grep dan los mismos llamadores.
- Se quitaron `NOMBRE_R00T`, `OsStr` y `OsStrExt` de `barrido.rs`. Quedaron sin uso.
- `docs/builds/informe-local-107.md:70` dice "se deja el acoplamiento por texto". Esa decisión ya no vale.

## 6. Preguntas para el planner

1. ¿Se agrega una rama `PlazoVencido` en `descarga.rs` para conservar el mensaje de hoy?
   Hoy: `infraestructura: timeout tras N s; contenedor X eliminado`.
   Ahora: `la descarga falló: error de infraestructura del oráculo: timeout tras ...`.
   Decisión: no se tocó el archivo. Queda para una tarea que lo permita.
2. ¿Qué texto lleva el motivo del limpio vencido? Decisión: `suite sin veredicto: la suite pasó su plazo`. Todo termina en `MOTIVO_SUITE_PLAZO`, que es lo que lee el código.
3. ¿Un limpio vencido se comparte con otras dimensiones? Decisión: no. Cada dimensión con juez lo corre de nuevo.
4. ¿El pid propio cuenta como r00t vivo en el barrido de `corre`? Decisión: sí, por la regla de `limpieza.rs`. El margen de 300 s sigue como barrera.
5. ¿`mismo_binario` basta para contar un pid como vivo en el barrido de credenciales? Decisión: sí. Un pid reusado por el mismo ejecutable lo corrige solo el margen.
6. Un limpio vencido sin junit pierde sus otros puntajes: su veredicto pasa a ser un clon de la base. Decisión: se acepta, porque ese limpio no mide.
