# Informe de la Tarea 120

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`, sumados todos los binarios.

- Base (3ad1b39): 3461 verdes, 0 fallas, 5 ignorados.
- Final: 3471 verdes, 0 fallas, 5 ignorados.
- El hook corrió en cada commit. Ningún test de tiempo falló, así que no hubo reintentos.
- Los puntos 3 a 6 se construyeron en worktrees propios (`l120-p3` a `l120-p6`). Después pasaron a esta rama con `git cherry-pick`.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La calibración leía el digest con podman real | `calibra_tarea_usa_el_digest_del_lector_para_decidir_el_reuso` | de40220 |
| 2 | Dos tests lanzaban contenedores sin guarda | La guarda dinámica: 4 llamadas antes, 0 después | db21d0a |
| 3 | `podman_falso` daba ETXTBSY con forks concurrentes | `podman_falso_no_da_etxtbsy_con_fork_concurrente`, `sonda_de_podman_falso_no_deja_marca` | 92c0b4f |
| 4 | `medir` ignoraba el error del borrado | `medir_con_borrado_que_falla_deja_aviso_con_carpeta_y_error` | 88bd4d9 |
| 5 | El empate entre ids desconocidos dependía del vector | `informe_eficiencia_empate_entre_desconocidos_no_depende_del_vector` | ecd0c36 |
| 6 | La instrucción podía decir "sin valor veces" | `regresiones_de_vuelta_medida_sin_valor_no_entra_en_la_instruccion` | 6e788f6 |

Guardas sin rojo:

- Punto 1: `calibra_tarea_lee_el_digest_una_vez_en_la_base_y_una_por_caso`.
- Punto 3: la repetición de 200 pruebas con 4 hilos de fork.
- Punto 4: `medir_con_borrado_que_anda_no_deja_aviso_y_la_medida_no_cambia`.
- Punto 5: `..._gana_el_alfabetico` y `..._razon_mayor_gana_a_desconocido_alfabetico`.

## 3. Qué falló o quedó incompleto

- Punto 4: la ruta de `crear_trabajo` que falla al crear subcarpetas no tiene test. Se revisó por lectura.
- Punto 4: ningún test confirma que el aviso llegue al veredicto final. El cableado compila y los tests `oraculo_eficiencia_*` siguen verdes.
- Punto 5: el caso del brief (`zeta` primero) ya daba `alfa` antes del cambio. `max_by` devuelve el último del empate. El rojo real es el vector invertido (`alfa` primero).
- Punto 6: en producción el defecto es latente. `medida_de_eficiencia` marca `regresion` solo con dos valores. El tipo sí permite `regresion` sin valores.
- Punto 1: el lector se llama 1 + casos veces por calibración, no una vez. Ver pregunta 1.
- Punto 3: el rojo se reprodujo en el primer intento. No se midió cuántas de las 200 repeticiones fallan sin la prueba.
- Sin probar con podman: los dos tests con guarda nueva, `target::digest_de_imagen` real, y el aviso con un borrado real que falla. Los corre el orquestador.

Misma carrera ETXTBSY, sin cambiar:

- `src/corre/corrida.rs:3778`, su `podman_falso` (el brief citaba la línea 3436).
- `tests/corre.rs:233`.

## 4. La guarda dinámica

Un `podman` falso al frente del `PATH` anota cada llamada en `~/.cache/guarda120/llamadas.log`.

| Corrida | Llamadas |
|---|---|
| Auditoría de la 118, suite completa, antes | 216 en total, 41 tests de calibración incluidos |
| Punto 1, con un envoltorio que dejaba el defecto | 12 `podman image inspect` |
| `calibra --lib`, tras el punto 1 | 4, todas de `panico_en_entrada_lo_detecta_la_sonda_nativa` (gitleaks) |
| `--test calibra`, tras el punto 1 | 0 |
| Tras el punto 2, ese test y `oraculo_home` | 0 |
| Final, suite completa con `--test-threads=1` | 0 |

No medí el "antes" por test en esta tarea. Reuso el total de la auditoría 118 y las dos mediciones del builder.

## 5. Tests que cambiaron de valor

- Ningún test cambió su valor esperado.
- Punto 1: 42 llamadas en `src/oracle/calibra.rs` y 20 en `tests/calibra.rs` pasan a `calibrar_tarea_con_lector` con `digest_falso`. Cubren los 41 de la lista y 13 más que llamaban a `calibrar_tarea_con` o `calibrar_tarea`. Las dos llamadas de `calibrar_tarea` en `tests/calibra.rs` usan `.map(|c| c.fila)`.
- Punto 4: 7 llamadas a `medir` en `src/oracle/eficiencia.rs` suman `&mut Vec::new()`.
- Tests de otros archivos: ninguno. Ni `src/tipos.rs` ni `src/lib.rs` cambiaron.

## 6. Llamadores revisados

- `calibrar_tarea_con`: firma pública igual. Es un envoltorio que pasa `&target::digest_de_imagen`. Su llamador de producción, `src/lib.rs:188`, no cambia. Los de tests cambiaron a la función nueva.
- `calibrar_tarea_con_lector`: función nueva. Solo la llaman tests y el envoltorio.
- `calibrar_tarea`: sin cambios.
- `eficiencia::medir`: suma el parámetro `avisos: &mut Vec<String>`. Grep y codegraph coinciden: un llamador de producción (`src/oracle/mod.rs:840`) y 7 de tests. Codegraph suma `mod.rs:583`, que llama al `medir` propio de `oracle`. Es una colisión de nombres.
- `crear_trabajo` y `medir_lados`: un llamador cada una, `medir`.

## 7. Preguntas para el planner

1. **Digest una vez por calibración.** El brief pide que el lector se llame una vez. Hoy se llama en la base y en cada `evaluar_copia`. Decisión: no cambié la lógica de reuso del `target`. El test mide `1 + casos`.
2. **Firma de `calibrar_tarea_con`.** `src/lib.rs` está prohibido, así que no sumé el lector a la firma existente. Decisión: función nueva `calibrar_tarea_con_lector` y envoltorio. ¿Conviene mover `lib.rs` a la función nueva en una tarea futura?
3. **`medir` cambia de firma pública.** Decisión: parámetro de salida `avisos`, como el resto de `oracle/mod.rs`. Los tests de `medir` se tocaron.
4. **Texto del aviso del punto 4.** Decisión: `eficiencia: carpeta de trabajo sin borrar <ruta>: <error>`, en el estilo de "sin armar".
5. **Punto 6.** El filtro exige `razon`, `referencia` y `repo`, no solo `referencia`. Decisión: filtrar en `regresiones_de_vuelta`. `prompt_de_eficiencia` sigue aceptando una lista directa con medidas sin valor. Su test de la línea ~4147 lo afirma.
6. **Sin regresiones tras el filtro.** La lista queda vacía y `correr` no lanza la vuelta. Es lo que ya pasa sin regresiones. No lo cambié.
7. **Script con lista vacía.** El `podman_falso` de tests imprime un error de sintaxis de `sh` con lista vacía. No cambia los resultados. No lo toqué.
