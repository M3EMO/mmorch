# Informe de la Tarea 143

## 1. Resultado de la suite

- Base (1f17041): no corrí la suite aparte. El hook de pre-commit del punto 1 corrió la suite completa y dio verde.
- Final (`tools/test-seguro.sh`, `ROOT_SIN_PODMAN=1`): 3679 verdes, 0 fallas, 5 ignorados.
- Ningún test de tiempo falló en el hook. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El camino real de la sonda no tenía test de punta a punta. | Ninguno: dio verde (ver sección 4). Queda como guarda. | 7ecaf01 |
| 2 | La sonda marcaba `bootstrap/` y corría todos sus tests. | `sonda_lastre_con_sonda_marca_la_copia_del_repo_bueno_y_no_la_de_bootstrap`, `sonda_lastre_comando_lleva_el_filtro_de_los_tests_comunes`, `sonda_lastre_sin_filtro_de_tests_comunes_da_error_y_no_corre`, `sonda_lastre_funcion_nueva_del_repo_que_corre_queda_en_la_lista`, `sonda_lastre_comando_lleva_el_filtro_antes_de_la_marca`, `filtro_de_tests_comunes_lee_sin_vaciar_la_referencia` | 9d9319e |
| 3 | Sin lista de la sonda, el lastre usaba la regla de la 136 en silencio. | `sonda_lastre_sin_funciones_no_pone_lastre_y_lo_dice`, `..._con_error_...`, `..._sin_resultado_...`, `sonda_lastre_la_fila_guarda_lo_que_hizo_la_sonda`, `sonda_lastre_renglon_del_lastre_dice_que_lo_puso_la_sonda`, `sonda_lastre_lugar_por_nombra_quien_eligio_el_lastre`, `sonda_lastre_error_de_la_sonda_llega_a_los_avisos`, `lastre_sonda_funcion_nueva_que_corrio_es_candidata`. Guarda: lectura de una fila vieja. | 3397fd4 |

## 3. Qué falló o quedó incompleto

- La sonda real sobre s2-t1 y la calibración de t1 a t3 quedan para el orquestador (necesitan podman).
- No verifiqué que `nueva` sea elegida de hecho en el test de punta a punta. El test solo exige que ningún lastre caiga en `no_corre`.
- El hook impidió borrar scripts temporales de un subagente en `/tmp/t143`. Están fuera del repo.

## 4. La causa

- El test de punta a punta dio verde con el código de la base. Una mutación que vacía `corridas` en `calibra.rs:1790` lo pone en rojo. El test sí detecta la pérdida de la lista.
- Entonces la lista no se pierde en el camino que el test recorre: `con_sonda_lastre`, `evaluar_con`, `dejar_sonda_lastre`, `tomar_sonda_lastre`, `corridas_de_la_sonda`.
- La causa más probable es el punto 2: la sonda miraba `bootstrap/`. Sus funciones y sus archivos no coinciden con los del repo. Con `cambios` presentes, la lista no cubría los archivos tocados.
- El punto 3 cubre cualquier pérdida futura: sin lista, el caso queda no aplicable con el motivo.

## 5. Tests que cambiaron de valor

- `calibra_camino_caliente_sin_funciones_cuenta_no_aplicables`, `calibra_camino_caliente_con_funcion_existente_aplica_el_bucle`, `calibra_con_eficiencia_el_bucle_entra_a_la_rotacion`, `calibra_sin_eficiencia_el_bucle_no_entra_a_la_rotacion`: su evaluador no dejaba sonda. Sin sonda, el lastre ya no tiene lugar. Ahora usan `con_sonda_de_todas`. Sus aserciones no cambiaron.
- Los tests `sonda_lastre_*` de `mod.rs` usan `EficienciaFalsa` para lo que no es la sonda: la sonda ahora necesita la medición de Eficiencia. Sus valores esperados no cambiaron.
- Tests de otros archivos que edité: `src/oracle/mod.rs` (literales de `calibra::Fila`; la lista permitida del test de punta a punta pasa a `uno`, `dos`, `nueva`) y `tests/calibra.rs` (3 literales de `Fila`).

## 6. Llamadores revisados

- `cmd_sonda(filtro)`, `correr_sonda_de_lastre` y `medir_lastre`: solo los llama `evaluar_con` y sus tests.
- `eficiencia::filtro_de_tests_comunes`: función nueva, la llama `filtro_de_la_sonda`.
- `Fila` y `CasoGuardado` ganan un campo: `cargo check --all-targets` no reporta literales sin él.
- No corrí `codegraph`. Cubrí los llamadores con `grep -w` y con el compilador.

## 7. Preguntas para el planner

1. **Filtro vacío.** El brief pide un error con "filtro vacío". Un filtro `""` significa que todos los tests son comunes. Decisión: lo acepto como válido. Solo `None` o `Err` dan error.
2. **Regla de la 136.** Hoy no se usa en producción: solo Eficiencia tiene lastre y siempre pide la sonda. El valor `nombre` de `lugar_por` solo se prueba en un test unitario. Decisión: la dejo. ¿La quitamos?
3. **Sonda que no corrió (`None`).** La fila guarda `corrio: false` y `motivo: None`, y no hay aviso. El motivo "la sonda no corrió" aparece solo en el caso no aplicable. ¿Debe avisar también?
4. **Aviso.** No sumé `avisos` a `Medida`. El aviso sale de `fila.sonda_lastre.motivo`. Hay una sola fuente.
5. **Regla de la 134.** Su cambio vive en `inyectar.rs`, que la tabla asigna al punto 3. Va en el commit del punto 3.
