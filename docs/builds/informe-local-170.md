# Informe de la Tarea 170

Base: 31965b2. Rama: `laptop/tarea-170`. No usé `codegraph`: busqué con `grep -rn`. Todo corrió con `ROOT_SIN_PODMAN=1`.

## 1. Resultado de la suite

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (31965b2) | no la medí por separado | — | — |
| Final | 4098 | 0 | 5 |

- La base no tiene corrida propia sin podman. El informe de la Tarea 169 dio 4098/0/5 con podman.
- Los 5 ignorados son los de siempre: `target_podman`, `builder_real`, descarga de Python y la importación de métricas.
- El hook de pre-commit corrió la suite entera y pasó al primer intento. No hubo test de tiempo fallido.

## 2. Commits

| Punto | Tests vistos en rojo | Commit |
|---|---|---|
| 1 y 2, juntos | `juez_p170_huella_es_la_de_2020ecc`, `juez_p170_sistema_dice_parte_en_0`, `juez_p170_sistema_sin_escala_de_0_a_3_ni_regla_del_tema`, `huella_p170_las_tres_del_juez_son_las_de_2020ecc`, `huella_p170_fila_de_diseno_con_la_huella_nueva_calibra`, `huellas_de_las_nueve_dimensiones_son_fijas` | eed807b |

Guardas que siguen verdes: `juez_p169_fase_prompt_es_juez` (la fase `"juez"`), los otros `juez_p169_*` y `huella_p169_las_seis_de_oraculo_no_cambian`.

## 3. Qué falló o quedó incompleto

- **Un solo commit para los puntos 1 y 2.** El hook corre la suite entera. Las guardas del punto 2 fallan sin el prompt del punto 1, y las viejas fallan con él. Ningún commit por separado pasa el hook. Sin `--no-verify`, los dos puntos van juntos.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales. Los corre el orquestador.
- No corrí podman. La suite con podman queda para el orquestador.

## 4. Las huellas

| Dimensión | Base (31965b2) | Final |
|---|---|---|
| Funciona | b4ec060a4cdeb4dc | b4ec060a4cdeb4dc |
| Seguridad | 6b15c7c7291f637e | 6b15c7c7291f637e |
| Eficiencia | 5aa7d3c6dcb51df4 | 5aa7d3c6dcb51df4 |
| Completitud | ce508b6314e2b5ca | ce508b6314e2b5ca |
| Entrega | 8a9975311af332d3 | 8a9975311af332d3 |
| Pruebas | cd85103d4cfb9aa1 | cd85103d4cfb9aa1 |
| Estructura | 814e3901efc1c7cf | afbc695e7b008336 |
| Arquitectura | 0f151f7fa43547c3 | 60c5fb9b0d576398 |
| Diseño | 9b2a621ebb5c4632 | 19b81dea4119d429 |

`huella_del_prompt()`: base `542f79459012b68d`, final `9263145aa4df4c7a` (la de 2020ecc).

## 5. Tests que cambiaron de valor o de nombre

- `juez_p169_huella_cambia_respecto_de_la_base` pasó a `juez_p170_huella_es_la_de_2020ecc`. Pide igualdad con `9263145aa4df4c7a`.
- `juez_p169_sistema_no_dice_parte_en_0` pasó a `juez_p170_sistema_dice_parte_en_0`. Pide que el texto contenga "parte en 0".
- `juez_p169_sistema_dice_escala_de_0_a_3_y_regla_del_tema` pasó a `juez_p170_sistema_sin_escala_de_0_a_3_ni_regla_del_tema`. Pide que el texto no contenga esas frases.
- `huella_p169_las_tres_del_juez_cambian` pasó a `huella_p170_las_tres_del_juez_son_las_de_2020ecc`. Pide igualdad.
- `huella_p169_fila_de_diseno_con_la_huella_de_la_base_no_calibra` pasó a `huella_p170_fila_de_diseno_con_la_huella_nueva_calibra`. La huella de 2020ecc calibra. La de la 169 no calibra.
- `huellas_de_las_nueve_dimensiones_son_fijas` (`src/colab_calibra.rs`) vuelve a los literales de 2020ecc en las tres del juez.
- **Fuera de la lista del brief:** `huella_del_juez_no_cambia_con_el_reintento` (`src/oracle/juez.rs`, test) fijaba los literales de la 169. Cambié solo esos tres literales. Nombre y lógica quedan iguales.
- Quedó en `juez.rs` la constante de test `BASE_BYTES`, que usa `juez_p169_prompt_crece_menos_de_1024_bytes`. No cambió.

## 6. Preguntas para el planner

1. **Un commit o dos.** El brief pide un commit por punto. El hook de la suite lo impide, porque cada punto rompe los tests del otro. Decisión: un commit para los dos puntos, anotado arriba. El planner puede pedir que el brief de tareas con guardas cruzadas lo diga.
