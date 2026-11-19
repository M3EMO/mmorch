# Informe de la Tarea 135

## 1. Resultado de la suite

Todas las cifras usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Lib: verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (eed1225) | 3036 | 0 | 1 |
| Final (punto 4) | 3047 | 0 | 1 |

- La suite completa final no tiene fallas. Los tests de integración quedan verdes.
- Los 11 tests nuevos de la lib explican la diferencia.
- Un test con podman queda con `saltar_sin_podman`. Esta tarea no agrega ninguno.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 5 | Un lastre sin lugar usa el motivo común | `motivo_de_lastre_con_rangos_sin_lugar_lo_dice` | 507b641 |
| 1 | La medida no guarda las corridas | Tres tests que no compilaban: corridas de cada lado, referencia más ancla, medida vieja sin listas | 5ccf7f4 |
| 2 | El ruido sale solo de la referencia y `tiempo_tests` corre 3 veces | `comparar_ruido_del_repo_mayor_que_el_de_la_referencia_gana`, `medir_corre_los_tests_de_cada_lado_corridas_tests_veces` | 0d3d697 |
| 3 | Un caso compila el repo 3 veces | `con_referencia_el_repo_compila_una_vez_y_el_inicial_una_su_ancla` | 90e2c04 |
| 4 | Un ancla que falla no deja aviso | `con_referencia_el_ancla_con_error_deja_aviso_por_medida` | 8800f71 |

Guardas que no estaban en rojo:
- `comparar_lados_estables_y_50_por_ciento_mas_lento_es_regresion` (punto 2).
- `sin_referencia_cada_lado_compila_tres_veces` (punto 3).
- `con_referencia_un_ancla_sana_no_deja_aviso_de_ancla` (punto 4).
- La lectura de una medida vieja sin listas (punto 1).

## 3. Qué falló o quedó incompleto

- Los cinco puntos están hechos.
- El punto 1 exigió una edición de producción en `src/oracle/mod.rs`, un archivo prohibido. Ver la pregunta 1.
- El orquestador debe correr la calibración real con podman. Este informe no la prueba.
- El punto 1 no prueba el efecto real sobre el ruido de t2 (razón 1,024). Esa prueba también es del orquestador.

## 4. Las llamadas de un caso

Un caso con referencia fijada llama al ejecutor en este orden.

Repo, 1 metadata y 1 lista.
Inicial, 1 compilación de tests (ancla). Repo, 1 compilación de tests.

| Orden | Lado | Llamada | Corridas |
|---|---|---|---|
| 1 | repo | metadata | 1 |
| 2 | repo | lista | 1 |
| 3 | inicial | compilar tests (ancla) | 1 |
| 4 | repo | compilar tests | 1 |
| 5 | repo | tests | 1 |
| 6 | inicial | tests (ancla, tras la corrida 0) | 1 |
| 7 a 14 | repo | tests | 8 más |
| 15 | repo | build | 1 |
| 16 | inicial | build (ancla) | 1 |

- El repo corre 10 tests y 1 compilación. El inicial corre 1 test y 1 compilación.
- La base de la calibración y una evaluación normal corren 10 tests y 3 compilaciones por lado.
- La base mide además la metadata y la lista de cada lado.

## 5. Tests que cambiaron de valor

En `src/oracle/eficiencia.rs`:
- `comparar_ruido_bajo_usa_el_piso_y_marca_regresion`: el repo pasó a [1,6; 1,61; 1,6] para que el piso siga mandando. El valor esperado no cambió.
- `medir_alterna_los_lados_en_tests_y_compilacion`: los tests pasan de 3 a 10 por lado.
- `medir_compila_los_tests_una_vez_por_lado_antes_de_las_corridas`: igual, 3 a 10.
- `sin_referencia_el_orden_alterna_como_hoy`: igual, 3 a 10.
- `medir_deja_las_corridas_de_cada_lado_en_su_orden`: las listas pasan de 3 a 10 valores.
- `con_referencia_corridas_referencia_son_la_serie_mas_el_ancla`: igual, 3 a 10.
- `con_referencia_el_orden_de_las_corridas_es_repo_inicial_repo_repo`: los tests pasan a 10 en el repo. La compilación pasa de [repo, inicial, repo, repo] a [repo, inicial]. El nombre ya no describe el caso.
- `ancla_sube_el_ruido_y_quita_la_falsa_regresion`: el repo pasa de 1,05 a 1,04. Ese es el margen del punto 4.

Tests de otros archivos (solo literales de `MedidaEficiencia`, con las dos listas vacías):
- `src/tipos.rs`: dos literales de test.
- `src/corre/corrida.rs`: el ayudante `medida_eficiencia`.
- `src/oracle/informe.rs`: el ayudante `eficiencia`.
- `src/oracle/calibra.rs`: `medida_eficiencia_de_prueba`.

Producción fuera de la tabla: `src/oracle/mod.rs`, función `medida_de_eficiencia`, dos líneas que copian las listas.

## 6. Llamadores revisados

| Función | Cambio | Codegraph | Grep -w |
|---|---|---|---|
| `medir_compilacion` | suma `corridas` y agrupa las metadatas en un arreglo | `medir_los_dos_lados`, `medir_contra_referencia` | las mismas 2 llamadas |
| `medir_lados` | suma `avisos` | `medir_con` | 1 llamada |
| `medir_contra_referencia` | suma `avisos` | `medir_lados` | 1 llamada |
| `comparar` | el ruido suma la dispersión del repo | sin llamadores fuera de `eficiencia.rs` | igual |
| `comparar_con_ancla` | sin cambio de firma | `combinar` | `combinar` y 2 tests |
| `ResultadoMedida`, `MedidaEficiencia` | campos nuevos | literales en `mod.rs` y en tests | los de la sección 5 |

## 7. Preguntas para el planner

1. **La edición de `src/oracle/mod.rs`.** El punto 1 no compila sin dos líneas en `medida_de_eficiencia`. Sin ellas, las listas guardadas quedarían vacías. Decisión: edité solo esas dos líneas y lo anoté. La 136 no toca ese archivo, así que la mezcla no debe chocar.
2. **El ancla y el ruido en tests.** Con la regla del máximo, un ancla muy distinta de la base sube el ruido de `tiempo_tests` a un valor grande. Con un ancla de 9 s contra una base de 1,5 s, el ruido llegó a 5. Decisión: no lo limité, porque la regla del brief lo pide. Conviene decidir si un ancla atípica debe tener tope.
3. **Orden de las corridas con referencia fijada.** `corridas_referencia` guarda la base y luego el ancla, aunque en el tiempo el ancla corrió dentro del caso. Decisión: sigue el orden de la serie combinada, como pide el brief.
4. **Un nombre desfasado.** Dos tests conservan nombres que ya no describen su caso: `comparar_ruido_bajo_usa_el_piso_y_marca_regresion` conserva su sentido, pero `con_referencia_el_orden_de_las_corridas_es_repo_inicial_repo_repo` ya no. Decisión: no los renombré, para no ampliar el diff.
5. **Aviso por medida.** Un ancla que falla en `compilar_tests` deja dos avisos, uno por `tiempo_tests` y otro por `memoria_pico`. Decisión: un aviso por medida, como pide el brief.
