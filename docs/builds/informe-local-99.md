# Informe de la Tarea 99

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 7af9a54 | 3135 (derivado: final menos 11 tests nuevos) | 0 | 4 |
| Final | 3146 | 0 | 4 |

El número de la base no se corrió. Sale de restar los 11 tests nuevos del resultado final.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Un producto que no compila no daba "no listo". | `informe_no_compila_sin_bloqueos_da_no_listo_con_su_disparo`. Antes no compilaba el test, porque faltaba el campo. | aed4cec |
| 2 | El contrato con 101 bajo nextest daba "sin veredicto" en las nueve dimensiones. | `oraculo_no_compila_contrato_deja_funciona_en_cero_y_ocultos_fallados`, `..._no_corre_la_oculta_ni_llama_al_juez`, `..._mantiene_las_sondas_de_seguridad`, `..._sin_pruebas_esperadas_da_funciona_en_cero_con_veredicto`, `..._pruebas_con_mutantes_rotos_no_da_verde` | f604dc0 |

Guardas del punto 1:
- `informe_no_compila_con_secreto_da_bloqueado`
- `informe_veredicto_viejo_sin_no_compila_lee_false_y_no_cambia_la_decision`

Guardas del punto 2:
- `oraculo_no_compila_con_pytest_y_codigo_1_no_cambia`
- `oraculo_no_compila_nextest_contrato_verde_no_cambia`
- `oraculo_no_compila_oculta_101_con_contrato_compilado_no_es_no_compila`

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- No se corrió la evaluación real. El orquestador la corre sobre la copia de t1 de S2.
- El test de Pruebas usa un ejecutor falso donde cargo-mutants sale con 101. No prueba el build real.
- Un hook de commit tardó más de 120 s. Terminó bien. Ningún test de tiempo falló.

## 4. Dimensiones con un producto que no compila

| Dimensión | Resultado |
|---|---|
| Completitud | 0, con veredicto. Sale de los ocultos esperados en rojo. |
| Funciona | 0, con veredicto. |
| Entrega | Sin cambios. La construcción falla por su cuenta. |
| Pruebas | Sin veredicto, porque cargo-mutants no compila. No se tocó `mutacion.rs`. |
| Seguridad | Sigue. Gitleaks y cargo audit corren. |
| Eficiencia | Sin veredicto. Su sonda necesita compilar. |
| Estructura | Sigue. Las sondas nativas corren. |
| Arquitectura | Sigue. Las sondas nativas corren. |
| Diseño | Sin veredicto. El juez no corre y el aviso es "sin juez: el producto no compila". |

La decisión es "no listo" con la regla "el producto no compila". Un secreto da "bloqueado".

## 5. Tests que cambiaron de valor

- `nextest_no_compila_en_el_contrato_toma_solo_los_contrato`. El contrato con 101 ahora da `Suite::NoCompila`, no `Clasificada`. El test comprueba también los esperados con `esperados_de_requisitos`.

## 6. Llamadores revisados

- **`medir`** (parámetro nuevo `no_compila`). Grep -w: solo `evaluar_con` lo llama, en el mismo archivo. Las otras coincidencias son `--solo-medir` y otra `medir` en `auditar.rs`. Codegraph no tiene índice en este worktree, no se corrió `init`.
- **`Veredicto`** (campo nuevo). Literales sin `..Default`: `mod.rs` (`evaluar_con`) y `tipos.rs` (un test). Se agregó `no_compila`. Los literales de `aprende.rs` compilan sin cambios.
- **`sin_veredicto_o_no_compila`**. Un solo llamador: `evaluar_suite`. Para la suite oculta, pytest y 100 sin junit no cambia.
- **Aviso duplicado.** `evaluar_con` ahora omite los avisos repetidos de la oculta. Afecta a cualquier aviso repetido, no solo a este caso.

## 7. Preguntas para el planner

1. **Un contrato con 101 y sin tests esperados.** Marca `no_compila` igual. Decisión: sí, porque el 101 prueba que el build falla.
2. **Funciona con escenarios sin medir.** Con `no_compila`, solo el caso "no hay tests que medir" pasa a 0. Un Funciona sin veredicto por escenarios queda sin veredicto. Decisión: no tocarlo.
3. **Diseño sin juez.** Queda sin veredicto, no en 0. Decisión: sin veredicto, porque el juez no midió nada.
4. **Regla con clave.** La regla se llama "el producto no compila", sin prefijo `clave: `. Las otras reglas llevan clave. Decisión: seguir el texto del brief.
5. **`docs/interfaces.md`.** Se regeneró en esta rama. Se regenera otra vez al mezclar con la Tarea 97.
