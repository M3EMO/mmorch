# Informe de la Tarea 166

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 372af94 (dato del informe 165) | 4030 | 0 | 5 |
| Final | 4046 | 0 | 5 |

Los dos commits pasaron el hook al primer intento. No hubo fallas de tiempo.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Las cabeceras, el contexto y las líneas quitadas de un `tests.diff` oculto, y los `use` de Rust, daban huellas falsas. | `diff_cabecera_diff_git_no_es_huella`, `diff_cabeceras_menos_y_mas_y_hunk_no_son_huella`, `diff_linea_agregada_larga_es_huella`, `diff_linea_de_contexto_larga_no_es_huella`, `diff_linea_quitada_larga_no_es_huella`, `use_largo_de_rust_no_es_huella`. Guarda: `guarda_archivo_que_no_es_diff_se_lee_como_hoy` y los tests de `transcript.rs` de antes. | 4402270 |
| 2 | No había un comando para revisar un transcript guardado. | `revisar_lectura_con_huella_real_da_una_lectura_y_no_imprime_la_linea`, `revisar_lectura_transcript_limpio_da_cero_y_sin_marcas`, `revisar_lectura_transcript_inexistente_da_error_de_uso_con_la_ruta`, `revisar_lectura_tarea_sin_oculto_da_error_de_uso_con_la_ruta`. Guardas: `revisar_lectura_no_crea_ni_cambia_archivos_del_root_home` y 4 tests de clap. | f72d45d |

## 3. Qué falló o quedó incompleto

- Los 4 tests de clap del punto 2 no se vieron en rojo. El subagente cableó la variante antes de correrlos. Quedan como guardas.
- Sin podman: queda para el orquestador la suite con podman y `revisar-lectura` sobre el transcript real de b8d0db4058. Debe dar 0 marcas.
- No usé `codegraph`. Los llamadores salen de `grep -w`.

## 4. Qué líneas de un diff cuentan como huella

Un archivo de `oculto/` que termina en `.diff` o `.patch` se lee como diff. Solo cuentan las líneas agregadas, sin su `+`. Después rige el filtro de siempre: 40 caracteres o más, única, fuera del texto visible. Un `use ` o `pub use ` no cuenta.

| Línea (ejemplo falso) | ¿Cuenta? |
|---|---|
| `diff --git a/src/x.rs b/src/x.rs` | No: cabecera |
| `--- a/src/x.rs` y `+++ b/src/x.rs` | No: cabecera |
| `@@ -10,6 +10,20 @@ fn nombre()` | No: cabecera de hunk |
| `index 1a2b3c..4d5e6f 100644` | No: cabecera |
| `     assert_eq!(total(&pedido_de_contexto), 1234);` (con espacio) | No: contexto |
| `-    assert_eq!(total(&pedido_de_antes), 999);` | No: línea quitada |
| `+    assert_eq!(total(&pedido_de_prueba), 1234);` | Sí: cuenta como `assert_eq!(total(&pedido_de_prueba), 1234);` |
| `+use paquete::modulo::TipoConNombreLargoParaPasar40;` | No: es un `use` |

## 5. Tests que cambiaron de valor

- Ninguno cambió de valor. Todos los tests de antes siguen verdes.
- No edité tests de otros archivos.
- Edité `docs/interfaces.md`: agregué la fila de `corre revisar-lectura`, como pide el brief para las interfaces.

## 6. Llamadores revisados

- `Comando::Corre` cambió de forma: `tarea` y `config` son `Option`, y hay un campo `accion`. Llamadores con `grep -w`: `src/cli.rs` (`ruta()`) y `src/lib.rs` (el `match`). Los tests de integración `tests/cli.rs`, `tests/corre.rs` y `tests/sondas.rs` invocan `corre` y pasan.
- `armar`, `es_huella_candidata` y `revisar_transcript` no cambiaron de firma.
- Funciones nuevas: `contar_lecturas` y `ConteoLecturas` (`transcript.rs`), y `CorreAccion` (`cli.rs`).

## 7. Preguntas para el planner

1. El brief pide conteos de `huella` y `nombre`. El comando también cuenta `ruta`, que es el tercer motivo de `oraculo_leido`. Sin ese conteo, un transcript que solo lee el oráculo por ruta daría "sin marcas", un falso negativo. **Decisión:** el comando imprime `ruta` y `marcado` cuenta los tres motivos. El conteo de ruta no revela nada del oráculo. ¿Se quita?
2. `r00t corre` sin argumentos daba un error de clap. Ahora clap lo acepta y `lib.rs` da un error de uso (código 2) con el mismo sentido. **Decisión:** se acepta ese cambio de mensaje.
