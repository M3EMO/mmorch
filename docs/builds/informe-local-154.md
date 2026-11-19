# Informe de la Tarea 154

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (ae9296b) | 3896 | 0 | 5 |
| Final | 3905 | 0 | 5 |

Los 9 verdes nuevos son los tests de esta tarea. Los tests con podman quedan para el orquestador.
El hook de commit no falló por tiempo en ningún punto.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Las cifras de S2 no tenían nombre ni fuente | `corte_s2_convertido_da_1794614400`, `corte_s2_invalido_da_error_con_el_texto` | 9bd0ba0 |
| 2 | El gastado sumaba todo desde la época 0 | `gastado_s2_ignora_la_llamada_anterior_al_corte` (dio 101), `gastado_s2_sin_eventos_es_el_previo` (dio 0), `gastado_s2_cuenta_la_llamada_de_otra_corrida_despues_del_corte` y `gastado_s2_cuenta_la_reserva_abierta_y_no_duplica_la_cerrada` (rojos por el previo) | b8a6b51 |
| 3 | Las cifras no salían a la vista | `linea_gasto_s2_trae_las_cinco_cifras`, `con_juez_el_manifiesto_trae_las_cuatro_cifras` (no compilaban) | 3dc6fe8 |

Guardas del punto 3: `sin_juez_el_manifiesto_no_trae_gasto_s2` y `con_juez_y_tope_agotado_rechaza_el_paquete`.
El rojo de la guarda sin `--juez` no se vio por separado: va en el mismo lote que los otros dos.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- El punto 1 llevó un `#[allow(dead_code)]` temporal, porque el hook corre clippy con `-D warnings`. El punto 2 lo quitó.
- La línea de gasto sale por stderr, no por stdout. Ver la pregunta 1.
- No se probó la línea con el `eventos.jsonl` real. La prueba queda para el orquestador.

## 4. Las cifras de S2

- Viven juntas en `src/colab_calibra.rs`: `TOPE_S2_USD`, `GASTO_S2_PREVIO_USD` y `CORTE_S2`.
- `corte_s2_epoch` convierte el corte a segundos con el crate `time`. Un texto inválido da `ErrorOraculo::Infra`.
- `gasto_s2(home)` calcula las partes. `linea_gasto_s2` arma la línea.
- `empaquetar --juez` imprime la línea por stderr, antes de que `lib.rs` imprima `ROOT_SHA_PAQUETE`.
- El manifiesto con `--juez` trae `gasto_s2` (previo, corte, desde el corte, total y restante), junto a `tope_s2_usd`.
- Sin `--juez`, ni la línea ni `gasto_s2` salen.
- La importación de la 142 conserva el `ts`, así que el gasto que vuelve de Colab cuenta si es posterior al corte. No hubo que cambiar nada.

## 5. Tests que cambiaron de valor

El helper `eventos_con_api` ahora escribe el evento un segundo después del corte. Antes usaba `ts` 0.

- `gastado_s2_usd_suma_el_gasto_de_api`: de 6,55 a 13,10. Suma el previo.
- `con_juez_el_manifiesto_fija_tope_y_solo_sus_trabajos_lo_llevan`: de 18,45 a 11,90, por el mismo motivo.
- `con_juez_y_tope_agotado_rechaza_el_paquete`: el mensaje trae 31,55 gastados, no 25,00.
- Tests de otros archivos: ninguno editado.

## 6. Llamadores revisados

- `gastado_s2_usd` conserva su firma. Ahora es `#[cfg(test)]`, porque nadie de producción lo llama: `empaquetar` usa `gasto_s2`. Grep: solo tests de `colab_calibra.rs`.
- `armar_y_empaquetar` suma un parámetro. Su único llamador es `empaquetar`.
- `Manifiesto` y `DatosManifiesto` suman `gasto_s2`. Se actualizaron todos los literales de `colab_calibra.rs`. `lib.rs` y `docs/interfaces.md` no los nombran.
- `r00t esquema` no lista el manifiesto. No hubo nada que regenerar.
- Codegraph: no se corrió el índice. Se usó `grep -w` sobre `src`, con los resultados de arriba.

## 7. Preguntas para el planner

1. **La línea de gasto sale por stderr.** `lib.rs` imprime `ROOT_SHA_PAQUETE` y está prohibido. Un campo nuevo en `Resumen` rompería sus tests. Decisión: `empaquetar` imprime por stderr, así `--json` no se ensucia. Si el planner quiere stdout, la 155 toca `lib.rs`.
2. **La línea sale también cuando el paquete se rechaza** por tope agotado. Decisión: se deja, porque explica el rechazo.
3. **`gastado_s2_usd` quedó solo para tests.** Decisión: se conserva con `#[cfg(test)]`. El planner puede pedir su borrado y migrar los tests a `gasto_s2`.
