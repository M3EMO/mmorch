# Informe de la Tarea 158

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (e839d74): 3331 verdes, calculado como el final menos los 3 tests nuevos. No la corrí aparte.
- Final: 3334 verdes, 0 fallas, 1 ignorado en el crate principal. Los demás binarios de test quedaron en verde.
- El hook del commit pasó al primer intento. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `script_trabajo` escribía en el guion cualquier valor del entorno | `script_trabajo_rechaza_secreto_con_valor_sin_mostrarlo`, `falla_de_script_trabajo_no_deja_guion_a_medias` (rojo: no compilaban con la firma `String`) | a1767e0 |

Guardas verdes: `entorno_de_comando_de_trabajo_pasa_sin_error` y los tests de `ROOT_TRABAJO_CMD` de la 157.

## 3. Qué falló o quedó incompleto

Nada. La suite con podman queda para el orquestador. No corrí la validez, `corre`, `calibrar` ni la pasada reales.

## 4. Los llamadores de `script_trabajo`

- `correr_runtime` en `src/colab_calibra.rs`: calcula el guion con `?` antes de `colab::escribir`. El error `Uso` sale de `correr_runtime`, y no queda `trabajo-N.py`.
- Test `script_con_comillas_recupera_el_json_del_trabajo`: usa `.expect`.

## 5. Tests que cambiaron de valor

- `script_con_comillas_recupera_el_json_del_trabajo` usa `.expect("guion sin secretos")` por la nueva firma.
- No edité tests de otros archivos.

## 6. Llamadores revisados

Firma cambiada: `script_trabajo` (`String` pasa a `Result<String, ErrorOraculo>`).
- `grep -nw script_trabajo -r src`: la definición, `correr_runtime` y el test. Son los tres.
- codegraph: no lo corrí. El grep cubre una función privada de un solo archivo.

## 7. Preguntas para el planner

1. ¿Un valor secreto vacío debe pasar? Decisión: sí. Solo falla un nombre secreto con valor no vacío, como pide el brief.
2. La guarda vive en `revisar_entorno_sin_secretos`, una función aparte. Decisión: sí, para que otro guion la reuse.
