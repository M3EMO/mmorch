# Informe de la Tarea 111

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh --no-fail-fast`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 64361fa | 3326 | 0 | 4 |
| Final | 3341 | 0 | 4 |

Los tests con podman quedan saltados. El orquestador los corre al mezclar.
Ningún test de tiempo falló en el hook. No hizo falta reintentar un commit.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Una fila de una sola tarea con 5 casos calibraba. | `wilson_inferior_16_de_16_y_5_de_5`, `calibra_juez_cinco_de_cinco_en_una_tarea_no_calibra`, `calibra_juez_16_de_16_repartido_en_cuatro_tareas_con_la_misma_huella_calibra`, `fila_con_otra_huella_no_se_suma`, `fila_con_tarea_hash_de_otro_tarea_toml_no_se_suma`, `fila_sin_tarea_hash_no_calibra`, `oraculo_un_defecto_y_tres_limpios_bien_calibra`, `dimension_sin_casos_aplicables_da_motivo_sin_casos`, dos de la regla del 20 % | b6265c6 |
| 2 | La línea de `calibrar` no mostraba la cota de la versión. | `texto_version_de_juez_con_cinco_de_cinco_dice_la_cota_y_los_casos_que_faltan`, `texto_version_de_oraculo_dice_defecto_detectado_y_limpios_bien` | 9252818 |

Guardas que nacieron verdes: `oraculo_un_limpio_mal_no_calibra`, `oraculo_sin_limpios_con_veredicto_no_calibra`, `texto_version_de_juez_que_ya_llega_no_pide_casos`, `texto_version_de_oraculo_sin_defecto_o_con_limpio_mal_lo_dice`.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer. Ninguna parte quedó sin probar.
- Toqué archivos fuera de la tabla del brief. El hook corre la suite y bloquea el commit si falla.
  - `src/oracle/mod.rs`: solo las filas de prueba de 3 tests.
  - `docs/interfaces.md`: cambió la descripción de `Fila`. Va en el commit del punto 1.
  - `src/oracle/calibra.rs`: `tiene_items_de_juez` pasó a `pub(crate)` en el punto 2.
- La Tarea 110 también toca `mod.rs`. Un conflicto de mezcla es posible en esos tests.
- El esquema de `Fila` no cambió. Solo cambiaron descripciones.

## 4. Cómo se juntan las filas

- Huella: `huella_de(dimension)`. Una fila sin huella no cuenta, ni en dimensiones de oráculo.
- Vigencia: `tarea_hash` igual al hash actual del `tarea.toml`, e `inyecciones_distintas` presente.
- Una fila incompleta, con `no_aplica` o inestable no cuenta.
- Por tarea cuenta solo la fila más reciente que vale.
- Hash de la tarea evaluada: el que recibe `aplicar`.
- Hash de otra tarea: se calcula con `calcular_hash` sobre `ROOT_HOME/oraculos/<tarea>/tarea.toml`. Sin archivo, la fila no cuenta.
- Detectados = `round(sensibilidad × inyecciones_distintas)`. Limpios bien = `round(especificidad × limpios con veredicto)`.
- `aplicar` y `motivo_sin_calibrar` conservan su firma. Leen `root_home()`. Sus versiones `aplicar_en` y `motivo_sin_calibrar_en` reciben `home`.

## 5. Tests que cambiaron de valor

- Helper `fila()`: cuenta por defecto, con 16 defectos, 16 limpios, hash vacío y huella actual.
- `calibra_umbral_es_0_9_inclusive` pasó a `calibra_cota_de_wilson_decide_no_la_exactitud`: 16 de 16 calibra y 15 de 16 no.
- `calibra_n_dos_con_un_defecto_y_un_limpio_acertados_no_califica` pasó a `..._calibra_el_oraculo`: un oráculo calibra con un defecto y un limpio.
- `calibra_cuatro_defectos_o_cuatro_limpios_no_alcanzan_el_minimo` pasó a `calibra_cuatro_de_cuatro_no_alcanza_la_cota_de_wilson`, ahora con Diseño.
- `dimension_solo_de_sondas_sin_huella_sigue_calibrando` pasó a `fila_sin_huella_no_calibra_una_dimension_de_oraculo`: una fila sin huella ya no cuenta.
- `fila_sin_tarea_hash_sigue_calibrando` pasó a `fila_sin_tarea_hash_no_calibra`.
- `fila_vieja_sin_inyecciones_distintas_se_lee_none_y_calibra_como_hoy` pasó a `..._no_cuenta`.
- `muestra_suficiente_cuenta_inyecciones_distintas_no_defectos` pasó a `version_cuenta_inyecciones_distintas_no_defectos`. Dos tests de `muestra_suficiente` se borraron con la función.
- `motivo_de_otro_juez_aparece_con_huella_vieja`: el motivo de otra tarea pasa de ninguno a "ningún caso aplicable en las tareas medidas".
- `calibra_global_promedia_solo_calibradas_con_veredicto`: la fila de Diseño tiene sensibilidad 0,5 para que no calibre.
- Reglas del 20 %: se quitó la aserción de muestra. Se sumaron dos tests sobre casos juntos.
- `tests/calibra.rs`, `eficiencia_no_aplicable_no_calibra_ni_entra_a_global`: la fila de Seguridad trae hash, inyecciones y huella.
- En `src/oracle/mod.rs`, tres tests de `oraculo_*` traen filas con hash, 16 casos y huella. Cambian dos expectativas:
  - `exactitud_balanceada` de Seguridad pasa de 0,95 a 1,0.
  - `exactitud_balanceada` de Diseño pasa de 0,7 a 0,75.
  - Ahora la exactitud sale de las sumas, no de la fila guardada.

## 6. Llamadores revisados

Codegraph y `grep -w` coinciden en todos los símbolos.

| Símbolo | Cambio | Llamadores |
|---|---|---|
| `UMBRAL` | Borrado | Solo `aplicar`. |
| `muestra_suficiente` | Borrado | `aplicar`, `motivo_sin_calibrar` y sus tests. |
| `MIN_DEFECTOS_APLICABLES`, `MIN_LIMPIOS` | Borrados | Solo `muestra_suficiente`. |
| `Calibraciones::aplicar` | Misma firma | `src/oracle/mod.rs:909`, `tests/calibra.rs` y tests de `calibra.rs`. |
| `Calibraciones::motivo_sin_calibrar` | Misma firma | `src/oracle/mod.rs:914`. |
| `tiene_items_de_juez` | Pasa a `pub(crate)` | `calibra.rs` y `src/lib.rs`. |

## 7. Preguntas para el planner

1. **Oráculo sin limpios con veredicto.** El brief dice "todos los limpios dieron bien". Con cero limpios eso es cierto sin probar nada. Decisión: un oráculo exige al menos un limpio con veredicto.
2. **Filas sin huella en dimensiones de oráculo.** El punto 10 las descarta. Las calibraciones viejas de Seguridad o Funciona pierden su calibración hasta recalibrar. Decisión: se descartan, como pide el brief.
3. **`aplicar` lee `root_home()`.** `oracle/mod.rs` está prohibido, así que no pasé `home` desde `evaluar_con`. Con `OpcionesOraculo.home` distinto de `root_home()`, las filas de otras tareas se buscan en el lugar equivocado. Decisión: `aplicar_en` existe y espera que `mod.rs` la llame con `home`.
4. **`inestables_de` no cambió.** Sigue aceptando filas sin `tarea_hash` y filas de sondas sin huella. Decisión: no tocarlo. ¿Debe usar la misma vigencia?
5. **Una fila por tarea.** Dos calibraciones de la misma tarea no se suman: cuenta la más reciente. Decisión: evita contar dos veces los mismos defectos. ¿Se prefiere sumar las corridas con semillas distintas?
6. **Casos que faltan.** La línea de `calibrar` cuenta los casos con acierto que faltan, por clase. Decisión: informar sensibilidad y especificidad por separado.
7. **Tests de `mod.rs` y la Tarea 110.** Corregí las filas de tres tests de `src/oracle/mod.rs` para pasar el hook. La Tarea 110 también toca ese archivo. Decisión: el orquestador resuelve un conflicto, si aparece.
