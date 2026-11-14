# Informe de la Tarea 124

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (b6245d9): la suite de la biblioteca tenía 2914 tests verdes, 0 fallas, 1 ignorado.
- Final: la suite de la biblioteca tiene 2918 tests verdes, 0 fallas, 1 ignorado.
- Los demás binarios de test quedan verdes, sin fallas.
- El hook de commit pasó al primer intento. No hubo reintento.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Una fila con `base_sin_veredicto` imprimía "sensibilidad sin medir ... casos 0". | `linea_sin_medir_base_sin_veredicto_dice_el_motivo` | 68af5ca |

Guardas, verdes desde el inicio:

- `linea_sin_medir_inestable_usa_el_formato_de_siempre`
- `linea_sin_medir_no_aplica_usa_el_formato_de_siempre`
- `linea_sin_medir_fila_medida_da_none`

La función pura es `linea_sin_medir` en `src/lib.rs`. La salida `--json` no cambia.

## 3. Qué falló o quedó incompleto

Nada. No se corrió ninguna calibración real, como pide el brief.

## 4. Tests que cambiaron de valor

Ninguno.

## 5. Preguntas para el planner

1. La línea `casos: <ruta>` se imprime solo con `base_sin_veredicto`. Las filas `inestable` y `no_aplica` no la imprimían antes y siguen igual. Decisión: no cambiar su salida. El planner decide si esas filas también deben mostrar la ruta.
