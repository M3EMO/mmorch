# Informe de la Tarea 116

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base cfb8970 | 3424 | 0 | 5 |
| Final de20fdc | 3444 | 0 | 5 |

El hook de commit corre la suite. Ningún test de tiempo falló, así que no hubo reintento.
Los tests con `saltar_sin_podman` salen como saltados. El orquestador los corre al mezclar.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Nada decidía cuándo hay vuelta de eficiencia. | `regresiones_de_vuelta_regresion_de_1_3_veces_da_vuelta`, `regresiones_de_vuelta_solo_devuelve_las_medidas_con_regresion`. Los otros cinco son guardas: pasan con el stub vacío. | 7df73d0 |
| 2 | `corre` no devolvía el trabajo al builder tras evaluar. | `corre_eficiencia_evaluador_con_regresion_y_despues_ninguna_lanza_dos_veces_el_ultimo_rol_y_evalua_dos_veces`, `..._la_instruccion_de_la_vuelta_nombra_medida_razon_y_numeros`, `..._evaluador_que_siempre_da_regresion_evalua_dos_veces_no_tres`, `..._sin_cupo_para_la_vuelta_la_fila_lleva_la_primera_evaluacion_y_el_gate_nombra_el_motivo`, `..._la_copia_de_la_credencial_no_existe_al_terminar_tambien_si_la_vuelta_falla`, `..._la_vuelta_con_codigo_distinto_de_0_deja_la_primera_evaluacion`, `..._segunda_evaluacion_con_error_deja_la_primera`. Guarda: `..._sin_regresion_lanza_y_evalua_una_sola_vez`. | 765a42d |
| 3 | La fila no guardaba la primera regresión. | `corre_eficiencia_la_fila_lleva_la_primera_regresion_en_la_fila_y_en_runs_jsonl`, `corre_eficiencia_sin_cupo_para_la_vuelta_lleva_corrio_falso_y_el_motivo`. Guardas: `fila_vieja_sin_vuelta_eficiencia_se_lee`, `fila_con_vuelta_eficiencia_hace_round_trip`, `corre_eficiencia_sin_regresion_la_fila_no_lleva_vuelta`. | de20fdc |

Los tres puntos tocan `src/corre/corrida.rs` y van en serie. No hubo subagentes en paralelo.
Un Haiku construyó cada punto. El organizador revisó cada diff antes del commit.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- Sin podman no se probó la vuelta con el ejecutor real de contenedor. Los tests usan el ejecutor falso.
- `src/corre/mod.rs` no cambió.
- El struct `VueltaEficiencia` es nuevo en `src/tipos.rs`, junto a `Fila`. No está en `r00t esquema`, igual que `MedidaEficiencia`.

## 4. La credencial en la vuelta

- **Dónde se arma.** El ejecutor real copia la credencial en cada `lanzar` (`EjecutorContenedorBuilder::preparar_credencial`, `src/corre/ejecutor.rs`). Root no la copia.
- **Qué guardia la cubre.** `correr_vuelta_eficiencia` crea su propia `GuardiaCredencial` antes de lanzar. Su `cerrar()` quita la copia tras revisar el transcript. Su `Drop` la quita en cualquier otra salida.
- **Cómo lo prueban los tests.** El test `..._la_copia_de_la_credencial_no_existe_al_terminar_tambien_si_la_vuelta_falla` cubre dos casos. En el primero la corrida termina bien. En el segundo la vuelta falla después de montar la copia. En ambos la copia no existe al final.

## 5. Tests que cambiaron de valor

- Ningún test existente cambió una aserción.
- Test de otro archivo: el literal `fila()` en el módulo de tests de `src/tipos.rs` suma `vuelta_eficiencia: None`. Sin eso no compila.
- El `Builder` de pruebas de `corrida.rs` suma los campos `falla_desde` y `falla_tras_montar_desde`. Los tests viejos los dejan en `None`.

## 6. Llamadores revisados

No cambió ninguna firma pública ni se borró ningún símbolo.
El paso e, el g y el i salieron a funciones privadas de `corrida.rs`. Las dos listas de llamadores se unieron.

| Símbolo | Llamadores |
|---|---|
| `lanzar_rol` | `correr_interno` (grep, línea 329) y `correr_vuelta_eficiencia` (grep y codegraph). |
| `revisar_transcripts` | `correr_interno` (grep, línea 364) y `correr_vuelta_eficiencia` (grep y codegraph). |
| `evaluar_repo` | `correr_interno` (grep, línea 374) y `correr_vuelta_eficiencia` (grep y codegraph). |
| `correr_vuelta_eficiencia` | `correr_interno` (grep y codegraph). |
| `regresiones_de_vuelta`, `prompt_de_eficiencia` | `correr_interno`, `correr_vuelta_eficiencia` y sus tests (grep). Codegraph no los indexó antes de `sync`. |

Codegraph no lista a `correr_interno` como llamador de los tres primeros. Solo grep lo ve. Por eso la unión importa.
`Fila` suma un campo con `#[serde(default)]`. Los literales de `Fila` de `src/aprende.rs` compilan y la suite pasa.
Los literales de `Fila` de `calibra.rs` son de otro tipo.

## 7. Preguntas para el planner

1. **La segunda evaluación da error.** Decisión: la fila lleva la primera evaluación y un `Gate` de fase `eficiencia` anota el motivo. Un veredicto por defecto borraría los datos de la primera.
2. **La vuelta sale con código distinto de 0, agota sus vueltas cortadas o no deja `result` exitoso.** Decisión: la fila lleva la primera evaluación, `corrio: true` y el motivo. El brief solo nombra el caso sin cupo.
3. **El repo cambió pero la fila describe el estado anterior.** Si la vuelta falla a medias, el repo del builder ya no es el evaluado primero. Decisión: se acepta y la fila anota el motivo.
4. **El último rol no deja sesión legible.** Decisión: la vuelta se lanza sin `--resume`.
5. **Un error al revisar los transcripts de la vuelta o al quitar la copia.** Decisión: sigue como error de la corrida con `?`, igual que los pasos g y h.
6. **Texto de la instrucción.** Una medida sin valor se escribe "sin valor veces la referencia". Esas medidas no llevan regresión, así que no llegan a la vuelta.
