# Informe de la Tarea 102

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base cabb373 (final de la Tarea 101) | 3188 | 0 | 4 |
| Final | 3204 | 0 | 4 |

Ningún hook de commit falló por un test de tiempo. Ningún commit se reintentó.
Los cuatro ignorados son los mismos de la base.

## 2. Commits

| Grupo | Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|---|
| A | A1 | `analizar` no recordaba comentarios de bloque ni cadenas entre líneas | `limpia_bloque_de_comentario_de_tres_lineas_queda_en_blanco`, `limpia_cadena_cruda_de_varias_lineas_deja_el_medio_en_blanco`, `limpia_bloque_anidado_se_cierra_en_el_segundo_cierre`, `limpia_conserva_el_largo_en_bytes_con_caracteres_multibyte`, `limpia_cadena_comun_multilinea_deja_el_medio_en_blanco`, `sonda_unsafe_dentro_de_comentario_de_bloque_no_cuenta`, `aristas_use_dentro_de_comentario_de_bloque_no_cuenta`, `limpia_comilla_de_caracter_no_abre_cadena` | 158bc8c |
| A | A2 | El inyector tenía su propio `linea_limpia` y mutaba líneas dentro de `/* */` | `comportamiento_con_inicial_no_muta_lineas_de_comentario_de_bloque`, `comportamiento_con_inicial_no_muta_lineas_de_cadena_cruda`, `agregar_negacion_no_confunde_una_llave_de_caracter_con_la_del_bloque` | ab972b5 |
| A | A3 | `bucle` excluía `const fn`, `async fn` y `unsafe fn` | `bucle_reconoce_pub_crate_const_fn`, `bucle_reconoce_async_y_unsafe_fn` | d77c05d |
| B | B1 | Una suite oculta con código 101 no marcaba `no_compila` | `oraculo_no_compila_oculta_101_con_contrato_compilado_marca_no_compila` | 10f94ed |
| B | B2 | El aviso decía "1 mutantes" | No se hizo (ver sección 3) | — |
| C | C1 | `con_tests/` ganaba a `--repo` sin aviso | `calibra_pruebas_usa_con_tests_de_la_tarea_y_la_fila_lo_dice` (ampliado con el aviso) | eb2a90f |

Guardas que pasaban antes y después:
- A1: los 142 tests previos de `oracle::perfiles`.
- B1: `oraculo_no_compila_oculta_100_sin_junit_no_marca_no_compila`.
- C1: `calibra_pruebas_con_con_tests_sin_repo_cli_no_avisa` y `calibra_pruebas_con_repo_cli_sin_con_tests_no_avisa`.

El builder de B y C trabajó en los worktrees `local102-b` y `local102-c`. El organizador pasó sus commits con `git cherry-pick`.

## 3. Qué falló o quedó incompleto

- **B2 sin hacer.** El texto "mutantes que no estaban en la muestra" vive en `aviso_de_mas`, en `src/oracle/mutacion.rs:493`. Ese archivo no está en la tabla del brief. El arreglo es plural/singular en esa función. Sus tests están en `mutacion.rs:2088` y en `mod.rs:5234`.
- **C1 queda a medias de cara al usuario.** El aviso vive en el campo nuevo `Calibrada.avisos`. `oraculo_calibrar` en `src/lib.rs` (~línea 184) todavía no lo imprime. Sin ese cambio, la terminal no muestra el aviso. `src/lib.rs` está fuera de la tabla.
- **Limitación de A1.** Un literal `'{'` sigue contando como llave en el conteo de `analizar`, igual que antes.
- **Cambio de comportamiento de A2.** Se borró `es_comentario`. Una línea de código que empieza con `* ` ahora recibe candidatos. `igualdad_invertida` (modo sin inicial) sigue línea por línea, para no mover la semilla de la regla de siempre.
- **Corrección extra de A2.** `ediciones_de_condicion` medía el fin de la condición sobre la línea limpia. Con `'{'` antes de la llave del bloque, la edición salía rota. Ahora mide sobre la línea original.
- No se corrieron validez, `corre`, `calibrar` ni la pasada real. Las corre el orquestador.
- `docs/interfaces.md` no se regeneró. `Calibrada` no deriva serde, y ningún esquema serializado cambió.
- Quedó una carpeta temporal de codegraph en `~/.cache/root-wt/local102-cg-head.*`. Un hook bloqueó su borrado. El orquestador puede borrarla.
- Un subagente escribió un log en `/tmp/b1-suite.log`. Ningún test creó carpetas en `/tmp`.

## 4. Sondas que cambiaron

Las sondas de Rust leen `limpia` de `analizar`. Qué espera cada una:
- Seguridad (`rust.rs:72`): `tiene_token("unsafe")`. Los espacios no la afectan.
- Seguridad (`rust.rs:84`): busca `.unwrap()` y `.expect(` como subcadena.
- Estructura (`rust.rs:365`): `tiene_token("fn")` y conteo de `{ } ;`.
- Arquitectura (`rust.rs:587`): `declara_modulo` compara `limpia.trim()` con formas exactas. El `trim` quita los espacios.
- Arquitectura (`rust.rs:606`): usos de `use crate::`, con `contains(';')`. Los desplazamientos en bytes ahora valen.
- `mapa.rs:120-128`: `item_publico` con `trim_start`, `tiene_token` y conteo de llaves.

Los delimitadores `"`, `r#"` y `"#` quedan en `limpia`. Las sondas siguen viendo que hay una cadena.

Versiones de sondas (Tarea 89), en `version_de_sondas`:

| Dimensión | Antes | Ahora | Motivo |
|---|---|---|---|
| Seguridad | 1 | 2 | Usa `unsafe` y `unwrap` de `limpia` |
| Estructura | 1 | 2 | Usa el largo de funciones |
| Arquitectura | 2 | 3 | Usa aristas y `declara_modulo` |
| Diseno | 1 | 2 | Su juez lee el mapa, que se arma con `analizar` |

Completitud, Funciona, Eficiencia, Entrega y Pruebas no leen `analizar` y no cambian.
Las filas viejas de las cuatro dimensiones dejan de valer. Se recalibran sin API.
Ningún hallazgo de los fixtures cambió: los 142 tests previos de `oracle::perfiles` pasan sin edición.

## 5. Tests que cambiaron de valor

- `version_de_sondas_de_arquitectura_cambia_su_huella_y_no_la_de_seguridad`: Arquitectura pasa de 2 a 3. La huella de Seguridad se compara con la versión 2.
- `calibra_huella_de_diseno_no_cambia_de_valor`, ahora `calibra_huella_de_diseno_incluye_su_version_de_sondas_2`: espera el sufijo `sondas v2`, porque Diseno sube a 2.
- `oraculo_no_compila_oculta_101_con_contrato_compilado_no_es_no_compila`, ahora `..._marca_no_compila`: fijaba lo contrario de B1. Ahora afirma `no_compila: true`. Conserva el aviso, los ocultos fallidos y una sola corrida de la oculta.
- `agregar_negacion_envuelve_la_condicion_con_parentesis`: cambió la firma de `candidatos_de_linea`. El valor esperado sigue igual.

## 6. Llamadores revisados

- `linea_limpia` (borrada). Antes del cambio, grep: `inyectar.rs:681` y `:697` (`cuerpo_de`), `:1287` (`candidatos_de_linea`). Codegraph: `cuerpo_de` y `candidatos_de_linea`. Las dos listas coinciden. Ahora, grep da cero y codegraph dice "Symbol not found".
- `es_comentario` (borrada). Su único llamador era `candidatos_de_linea`.
- `limpiar_linea` (borrada). Su único llamador era `analizar`, en el mismo archivo.
- `cuerpo_de` (firma: recibe las líneas limpias) y `candidatos_de_linea` (firma: recibe `linea` y `limpia`). Los llamadores están en `inyectar.rs`: `comportamiento_con_inicial` y los tests.
- `analizar` (firma igual). Llamadores: `mapa.rs:77` y `rust.rs`.
- `Calibrada` (campo nuevo `avisos`). Los tres constructores están en `calibra.rs`.
- `sin_veredicto_o_no_compila` (sin cambio de firma). B1 se resolvió en `evaluar_con`, no en esa función.

## 7. Preguntas para el planner

1. **¿Se sube la versión de Diseno?** Diseno no tiene sonda de Rust. Su juez lee el mapa, y el mapa usa `analizar`. El mapa no entra a la huella del prompt. Decisión: se subió a 2 para no mezclar filas viejas y nuevas. Costo: una recalibración sin API.
2. **¿Se autoriza tocar `src/lib.rs` para imprimir `Calibrada.avisos`?** Decisión: no se tocó. El aviso solo lo ve el test.
3. **¿Se autoriza tocar `src/oracle/mutacion.rs` para B2?** Decisión: no se hizo. El brief lo ubica en `mod.rs`, pero el texto vive en `mutacion.rs`.
4. **¿Se acepta que una línea de código que empieza con `* ` reciba candidatos de mutación?** Decisión: sí. `es_comentario` ya no hace falta con el estado entre líneas.
