# Informe de la Tarea 130

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (5409dbe): la lib tiene 2988 verdes, 0 fallas y 1 ignorado. No sumé el total de los otros binarios.
- Final: 3567 verdes, 0 fallas y 5 ignorados en todos los binarios.
- La lib sube 4 tests por los 4 tests nuevos de `memo.rs`.
- El hook de commit pasó a la primera. Ningún test de tiempo falló.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Los envoltorios de memo ignoran la carpeta de `ejecutar_en` | `grabador_ejecutar_en_llega_al_real_con_la_misma_carpeta`, `reusador_ejecutar_en_con_comando_no_grabado_llega_al_real_con_la_misma_carpeta` | 99ef4e9 |

Quedaron como guardas, verdes desde antes:
- `grabador_ejecutar_en_anota_la_salida_en_el_memo`. El valor por defecto llama a `ejecutar`, que ya anota.
- `reusador_ejecutar_en_con_comando_grabado_devuelve_lo_grabado_sin_llamar_al_real`.
- Los tests de `memo.rs` anteriores.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- La medición de la ganancia con podman queda para el orquestador. Todas las corridas de mutación deben montar la misma carpeta `.../mutacion`.

## 4. Los envoltorios de producción

Busqué con `grep -n "impl EjecutorHerramienta for" src/`. Fuera de los tests hay tres.

| Tipo | Pasa `ejecutar_en` |
|---|---|
| `EjecutorContenedor` | Sí, ya lo implementaba |
| `EjecutorGrabador` | Sí, desde esta tarea |
| `EjecutorReusador` | Sí, desde esta tarea |

No apareció ningún otro envoltorio. No usé codegraph: el `grep` sobre el `impl` cubre el hallazgo.

## 5. Tests que cambiaron de valor

Ninguno. No edité tests de otros archivos.

## 6. Preguntas para el planner

Ninguna.
