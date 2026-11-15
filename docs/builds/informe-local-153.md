# Informe de la Tarea 153

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (47cb2a9) | 3888 | 0 | 5 |
| Final | 3896 | 0 | 5 |

Los 8 verdes nuevos son los tests de esta tarea. Los tests con podman quedan para el orquestador.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `registros/` de cada tarea entraba al paquete | `paquete_no_trae_registros_de_la_tarea`, `paquete_no_trae_registros_ni_colab_ni_evidencia` (ahora revisa `registros`). Guarda: `repo_con_carpeta_registros_la_lleva` | f805c2a |
| 2 | Tope de S2 en 0 pasaba al paquete con `--juez` | `con_juez_y_tope_agotado_rechaza_el_paquete`. Guarda: `sin_juez_y_tope_agotado_el_paquete_se_arma` | a5b432a |
| 3 | `--tope-usd` redondeaba al más cercano | `tope_usd_se_redondea_hacia_abajo` (daba 18.46) | 0ed9edc |
| 4 | Sin defecto: el lock ya estaba | Solo guarda: `agregar_gasto_espera_el_lock_de_eventos` | 2c97645 |
| 5 | `/oraculo/../x` pasaba la revisión de bwrap | `bwrap_oraculo_destino_con_punto_punto_da_error_que_lo_nombra` | adbc700 |
| 6 | `lista_de_carpetas` entraba en pánico sin nombrar el guion | `lista_ausente_nombra_el_guion` | 473ed5d |

## 3. Qué falló o quedó incompleto

Nada quedó sin hacer. El reintento por test de tiempo en el hook no hizo falta.
El manifiesto guarda `tope_s2_usd` sin redondear. Solo el `--tope-usd` de cada trabajo baja al centavo.

## 4. El lock de la importación

Ya estaba. `agregar_gasto` usa `con_eventos_bloqueados`, que usa `con_lock_exclusivo` sobre `eventos.jsonl.lock`.
Es el mismo helper de los demás escritores y lee y escribe bajo un solo lock.
No cambié código de producción. Dejé una guarda: con el lock tomado, la importación espera y no escribe.
Una mutación temporal sin lock puso la guarda en rojo. La revertí.

## 5. Tests que cambiaron de valor

- `paquete_no_trae_registros_ni_colab_ni_evidencia` ahora revisa `registros`. `Escenario::con_contrato` crea `oraculos/tarea-prueba/registros/x.stderr`.
- `lista_de_carpetas` cambió de firma en `tests/colab_preparar.rs`. Ese archivo es un test de la tabla, no de otro archivo.
- Ningún test de otro archivo cambió.

## 6. Llamadores revisados

| Firma | Codegraph | grep -w |
|---|---|---|
| `comando_de_trabajo` (firma igual) | `armar_manifiesto`, un test | `armar_manifiesto` (698), un test |
| `tope_hacia_abajo` (nueva) | sin llamadores previos | solo `comando_de_trabajo` |
| `revisar_destinos_bwrap` (firma igual) | no consultado | `argumentos_bwrap` (213) |
| `lista_de_carpetas` (test, firma nueva) | no consultado | solo `comparar_lista` |

## 7. Preguntas para el planner

1. **Mensaje del tope agotado.** Decidí que da el gastado con dos decimales y `TOPE_S2_USD` de 25 USD. El texto dice "no se arma el paquete".
2. **Épsilon del redondeo.** Decidí sumar 1e-9 antes del `floor`. Así 18.45 no baja a 18.44 por coma flotante.
3. **Manifiesto sin redondeo.** Decidí no tocar `tope_s2_usd` del manifiesto. El brief pide el redondeo solo en `--tope-usd`.
