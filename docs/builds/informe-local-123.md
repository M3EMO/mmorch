# Informe de la Tarea 123

## 1. Resultado de la suite

La suite corre con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (b6245d9), cifra final del informe 122 | 3478 | 0 | 5 |
| Final (83b29b9) | 3502 | 0 | 5 |

La Tarea 123 suma 11 tests nuevos. La diferencia es 24. No medí la base de nuevo, así que no explico los 13 restantes.
Ningún hook de commit falló. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La mutación corría siempre la muestra entera | `mutacion_con_muestra_de_base_*`: `corre_solo_los_muertos_y_vencidos`, `trae_los_catorce_con_estado_de_cada_uno`, `un_muerto_que_sobrevive_baja_el_valor`, `sin_muertos_no_corre_la_mutacion`, `pedido_que_falta_da_el_error_de_hoy`, `restaura_tras_f_y_tras_un_panico`, y `mutacion_archivo_y_linea_saca_los_dos_campos_del_nombre`. Guarda: `mutacion_sin_muestra_de_base_el_comando_es_el_de_hoy`. | 98d2e4f |
| 2 | Los casos corrían los 14 mutantes | `calibra_caso_con_defecto_corre_solo_los_muertos_de_la_base`, `calibra_caso_limpio_corre_solo_los_muertos_de_la_base`. Guardas: `calibra_base_corre_la_muestra_entera`, `calibra_casos_corren_con_plazo_fijo_y_la_base_sin_el`. | 83b29b9 |

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- No corrí tests con podman. El orquestador mide la ganancia de tiempo (527 s a unos 250 s) al mezclar.
- `la_suite_no_la_ejecuta` (calibra.rs) también pasa por `evaluar_copia`. Usa solo el veredicto. Ningún test propio verifica que el `-F` restringido no le afecte.
- La suite completa en la base no se midió de nuevo (ver sección 1).

## 4. El comando de un caso

Con base de 14 mutantes y 3 muertos, el `-F` nombra solo esos 3. Forma del comando:

```
cargo mutants --no-shuffle --jobs 1 --timeout <plazo> --exclude-re 'delete field .* from struct .* expression' --output /salida/mutants -F '^(?:<muerto 1>|<muerto 2>|<muerto 3>)$' -- --offline
```

Cada `<muerto>` es el nombre exacto del mutante, con la regex escapada. El test de calibra usa `-F '^(?:src/a.rs:1:1: replace x -> u8 with 0)$'`, con los metacaracteres escapados.
Sin muertos en la base, el comando no corre.

## 5. Tests que cambiaron de valor

Ningún test existente cambió de valor. Ninguno fijaba el `-F` entero de un caso.
No edité tests de otros archivos. Revisé `tests/calibra.rs`, `mutacion.rs`, `sondas.rs`, `imagen_oraculo.rs` y `ficha.rs`.

## 6. Llamadores revisados

No cambié la firma de una función existente. Agregué `con_muestra_de_base` en `mutacion.rs`.
- Grep `-w con_muestra_de_base`: `calibra.rs:1027` (`evaluar_copia`) y los tests de `mutacion.rs`.
- Codegraph: el worktree no tiene `.codegraph/`. No lo inicié por el disco. Un cero no prueba nada, y el grep cubre la función nueva.
- `evaluar_copia` de `calibra.rs` cambia de comportamiento, no de firma. Sus llamadores están en las líneas 1288 (limpio), 1379 y 1479 (defecto). Ninguno es la base.
- `calibra/defectos.rs` tiene otra `evaluar_copia`. No la toqué.
- `medir_mutantes` y `sonda_mutacion_en` conservan su firma. Sin el valor de hilo, el comando es el de hoy.

## 7. Preguntas para el planner

1. **Una muestra de base vacía.** El organizador decidió que `con_muestra_de_base` con una muestra vacía no fija nada. La mutación corre como hoy.
2. **Un mutante de la muestra que la base no trae.** El organizador decidió correrlo. Así no se copia un estado desconocido.
3. **Hallazgos de los vivos copiados.** El organizador decidió crearlos desde el nombre del mutante (`archivo:línea`). Así `vivos` y los hallazgos coinciden. Con una línea ilegible, la línea vale 0.
4. **El hueco del `Timeout`.** Un comentario en `medir_mutantes` lo describe y cita la decisión del planner.
