# Informe de la Tarea 121

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`, sumados todos los binarios.

- Base (ea04410): 3471 verdes, 0 fallas, 5 ignorados. El dato sale del informe de la Tarea 120.
- Final: 3484 verdes, 0 fallas, 5 ignorados.
- El hook corrió en cada commit. En el commit del informe falló `corre::ejecutor::tests::vencimiento_sigue_borrando_el_contenedor_por_nombre`, un test de tiempo. El reintento pasó.
- Cada punto lo construyó un subagente Haiku, en serie, en este worktree. Los tres tocan `calibra.rs`.
- No se corrió podman, ninguna API ni `~/.local/share/root/`.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La fila no guardaba lo que dijo cada juicio de la base. | `diseno_con_tres_juicios_guarda_tres_juicios_base_con_sus_niveles`, `juicio_base_con_faltante_guarda_el_item_y_su_motivo`. Guarda: `fila_vieja_sin_juicios_base_lee_lista_vacia`. | 10f3b7e |
| 2 | Un juicio sin veredicto se leía como inestabilidad, y el primero cortaba con `Infra`. | `diseno_con_segundo_juicio_sin_veredicto_no_mide_y_lo_dice`, `juicio_repetido_sin_veredicto_corta_antes_de_juzgar_otro`, `base_sin_veredicto_devuelve_fila_con_el_motivo_del_juicio_1`, `cuenta_no_suma_la_fila_con_base_sin_veredicto`, `inestables_de_salta_la_fila_con_base_sin_veredicto`. Guardas: `calibra_dimension_inestable_va_a_inestable_y_no_a_no_aplica`, `calibra_dimension_inestable_no_mide_casos_y_no_califica`. | 2914d71 |
| 3 | La fila no decía la causa de cada ítem inestable. | `causa_de_item_que_varia_nombra_el_minimo_y_el_maximo`, `causa_de_item_que_falta_nombra_el_juicio_y_el_motivo`, `causa_de_item_que_falta_sin_faltante_dice_sin_nivel`. Guardas: `fila_vieja_sin_causas_inestables_lee_mapa_vacio`, `diseno_con_tres_juicios_que_varian_guarda_la_causa_del_item`. | dae2a4f |

## 3. Qué falló o quedó incompleto

- Los tres puntos están hechos.
- `diseno_con_tres_juicios_que_varian_guarda_la_causa_del_item` se vio en rojo con un ítem inexistente. El builder cambió después el ítem a `diseno.robustez` y no repitió el rojo. Hoy actúa como guarda.
- `src/lib.rs` no muestra `base_sin_veredicto` en la salida humana. Una fila así imprime "sensibilidad -, casos 0". El archivo está fuera de la lista. Ver pregunta 1.
- Quedan sin correr los tests con `saltar_sin_podman` y la repetición de P101 a. Los corre el orquestador.

## 4. Los campos nuevos de la fila

| Campo | Tipo | Ejemplo |
|---|---|---|
| `juicios_base` | lista de `JuicioBase` | `[{"niveles":[{"item":"diseno.robustez","nivel":2,...}],"faltantes":[{"item":"diseno.errores","motivo":"sin evidencia que pase el gate"}],"sin_veredicto":null}]` |
| `base_sin_veredicto` | texto opcional | `"juicio 2 de la base sin veredicto: el juez no respondió"` |
| `causas_inestables` | mapa de ítem a texto | `{"diseno.robustez":"varía de 1 a 3","diseno.errores":"falta en el juicio 3: sin nivel"}` |

- Los tres llevan `#[serde(default)]`. Una fila vieja se lee.
- `JuicioBase` es un tipo nuevo con `niveles`, `faltantes` y `sin_veredicto`.
- `docs/interfaces.md` se regeneró en cada punto. El diff solo agrega.

## 5. Tests que cambiaron de valor

- `calibra_base_sin_veredicto_es_infraestructura` pasó a `calibra_base_sin_veredicto_da_fila_sin_casos`. Antes esperaba `Err(Infra)` y rc 3. Ahora espera `Ok` con `base_sin_veredicto` "juicio 1 ...".
- `calibra_base_sin_veredicto_dice_el_motivo` lee el motivo de `fila.base_sin_veredicto`, no del error. El texto del motivo no cambió.
- `tests/calibra.rs`: `eficiencia_con_bloque_mantiene_la_medicion_previa` esperaba el error de la base. Ahora espera una fila sin casos, y el archivo se escribe.
- Tests de otros archivos con edición mecánica: se agregó el campo nuevo a literales de `Fila` en `src/oracle/mod.rs` (3 sitios) y `tests/calibra.rs` (3 sitios), en cada punto. No hay lógica nueva.

## 6. Llamadores revisados

No cambió la firma de ninguna función existente. Se sumaron `causas_de`, `motivo_de_juicio_base` y `juicio_base_de`.

- `calibrar_con`, `calibrar_medida_con_hilos`: grep -w da solo `calibra.rs` y `src/lib.rs:188` (`calibrar_tarea_con`). Cambió su resultado ante una base sin veredicto, de `Err` a `Ok`.
- `cuenta`: lo llama solo `version_en`. Ahora excluye `base_sin_veredicto`.
- `Calibraciones::inestables_de`: el llamador de producción es `src/oracle/mod.rs:1774`. Ahora salta las filas con `base_sin_veredicto`.
- `inestables::inestables_de`: lo llaman `calibra.rs` y `causas_de`. La regla no cambió.
- `base_sin_veredicto`, `juicios_base` y `causas_inestables`: nadie los lee fuera de `calibra.rs` y sus tests.
- No se corrió `codegraph`. El grep -w cubre los nombres, y ninguna firma existente cambió.

## 7. Preguntas para el planner

1. **Salida humana de `calibrar`.** `src/lib.rs:204` muestra el motivo solo con `inestable` o `no_aplica`. Una fila con `base_sin_veredicto` parece medida. Decisión del organizador: no tocar `lib.rs`, que está fuera de la lista. Conviene que el planner pida una tarea chica.
2. **`inestables_de` y una base sin veredicto.** La fila nueva no tiene `inestables`. Pisaba los inestables de la medición anterior. Decisión: `inestables_de` salta esas filas. Es parte del punto 2 y lleva test en rojo.
3. **`motivo_sin_calibrar_en`.** Para una fila `base_sin_veredicto` responde "ningún caso aplicable en las tareas medidas". Es engañoso. Decisión: no cambiarlo. Falta decidir si merece un motivo propio.
4. **Una fila por cada base sin veredicto.** Antes el error no agregaba fila. Ahora cada intento suma una fila a `calibracion.json`. Decisión: seguir el brief, que pide escribir la fila.
5. **`informe.rs` no lee `causas_inestables`.** Está fuera de la lista. Decisión: solo guardar el dato.
6. **"sin nivel" cubre dos casos:** el ítem falta en el puntaje, o falta el puntaje de la dimensión. Decisión: un solo texto, como pide el brief.
7. **La causa "varía de a" no dice qué juicio dio cada nivel.** Ese dato vive en `juicios_base`.
8. **Arquitectura de s2-t1.** Con 6 de 10 de peso, la base queda sin veredicto y ya no es error de infraestructura. La regla de dos tercios sigue igual. El planner decide con la evidencia nueva.
9. **Texto histórico** en `docs/builds/brief-local-68.md:50` y en el plan (líneas 3207, 4300, 4495) cita el error viejo de la base. No se tocó.
10. **Ítem inexistente en un test.** `diseno_con_tres_juicios_guarda_tres_juicios_base_con_sus_niveles` usa `diseno.acoplamiento`, que no está en `rubricas/diseno.toml`. Solo prueba `juicios_base`, así que no se cambió.
