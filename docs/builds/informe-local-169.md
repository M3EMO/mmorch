# Informe de la Tarea 169

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (a9d8441), solo la lib | 3466 | 0 | 1 |
| Final (ed2f783), suite entera | 4098 | 0 | 5 |

- La suma de la base entera no se midió. Solo se contó la lib.
- La lib final tiene 3477 verdes y 1 ignorado, antes de las 3 guardas del punto 3.
- El hook de pre-commit falló una vez por `cargo fmt` en el punto 1. Se corrió `cargo fmt` y el commit pasó. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El prompt dice "parte en 0" y contradice las anclas. El juez no tiene un solo camino por ítem. | `juez_p169_huella_cambia_respecto_de_la_base`, `juez_p169_sistema_no_dice_parte_en_0`, `juez_p169_sistema_dice_escala_de_0_a_3_y_regla_del_tema` | 4652960 |
| 2 | `FASE` dice `juez-diseno`, pero una llamada puntúa tres dimensiones. | `juez_p169_fase_prompt_es_juez` | c79f117 |
| 3 | Sin defecto. Solo guardas de las huellas. | Ninguno: son guardas | ed2f783 |

Guardas del punto 1: nivel 3 con cita real, nivel 3 sin evidencia, `sin_evidencia`, truncada, prompt determinista y crecimiento menor a 1024 bytes.
Guarda del punto 2: `juez_p169_fase_gasto_s2_cuenta_la_llamada_por_via`.
Guardas del punto 3: `huella_p169_las_seis_de_oraculo_no_cambian`, `huella_p169_las_tres_del_juez_cambian`, `huella_p169_fila_de_diseno_con_la_huella_de_la_base_no_calibra`.

## 3. Qué falló o quedó incompleto

- No hay puntos sin hacer.
- Sin probar, por falta de podman y de API: la suite con podman y la medición real de Diseño, Arquitectura y Estructura. Las corre el orquestador.
- Un subagente corrió `cargo` en el punto 1 y bajó dos crates. No se tocó `Cargo.toml`.

## 4. Las huellas

| Dimensión | Base | Final |
|---|---|---|
| Funciona | b4ec060a | b4ec060a |
| Seguridad | 6b15c7c7 | 6b15c7c7 |
| Eficiencia | 5aa7d3c6 | 5aa7d3c6 |
| Completitud | ce508b63 | ce508b63 |
| Entrega | 8a997531 | 8a997531 |
| Pruebas | cd85103d | cd85103d |
| Estructura | afbc695e | 814e3901 |
| Arquitectura | 60c5fb9b | 0f151f7f |
| Diseño | 19b81dea | 9b2a621e |
| Prompt del juez | 9263145a | 542f7945 |

Son diez filas: las nueve dimensiones y la huella del prompt.

## 5. Tests que cambiaron de valor

- `huella_del_juez_no_cambia_con_el_reintento`, en `src/oracle/juez.rs`: sus literales de prompt, Diseño y Arquitectura pasan a los valores finales. Motivo: el prompt cambió.
- **Otro archivo:** `colab_calibra::tests_importar::huellas_de_las_nueve_dimensiones_son_fijas`, en `src/colab_calibra.rs`. Cambian los literales de Estructura, Arquitectura y Diseño. Solo el test; producción intacta.

## 6. Llamadores revisados

- No cambió ninguna firma. Se cambió un literal (`FASE`) y un texto (`PROMPT_SISTEMA`).
- No se usó `codegraph`: no está instalado. Se usó `grep -w`.
- `grep -rn '"juez-diseno"'` en `src/`, `tests/` y `tools/`: solo la definición de `FASE` en `src/oracle/juez.rs`. Ningún lector filtra por esa fase.
- `grep -w FASE`: las otras dos constantes (`ficha/juez.rs`, `oracle/auditar.rs`) son propias y no cambian.
- `CORRIDA_JUEZ_DISENO` en `src/oracle/mod.rs` no cambia.

## 7. Preguntas para el planner

1. El test `juez_p169_fase_gasto_s2_cuenta_la_llamada_por_via` deja escrito que las fases `juez` y `juez-diseno` cuentan igual en el gasto de S2. Decisión: se dejó así, porque la regla P112 a mira `via`. Los eventos viejos con `juez-diseno` siguen contando.
2. `huellas_de_las_nueve_dimensiones_son_fijas` conserva su nombre aunque tres huellas cambiaron. Decisión: no se renombró, para no tocar más de un test ajeno.
