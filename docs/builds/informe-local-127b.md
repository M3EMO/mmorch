# Informe de la Tarea 127b

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Final (301f586), suma de todos los binarios | 3536 | 0 | 5 |

- El hook de pre-commit corrió la suite entera. No falló ningún test de tiempo.
- La suma de la 127 también fue 3536. Los dos tests nuevos entran en la lib; no verifiqué el desglose por binario.

## 2. Commits

| Punto | Defecto | Test visto en rojo | Commit |
|---|---|---|---|
| 1 | cargo-mutants 27 rechaza `--in-place` con `--jobs` | `mutacion_con_carpeta_fija_la_corrida_no_lleva_jobs`. Guarda: `mutacion_sin_carpeta_fija_los_comandos_son_los_de_hoy` | 301f586 |

- `comando_de_corrida` omite ` --jobs 1` si `hay_carpeta_fija()`. Lleva un comentario de una línea con la razón.
- El listado (`comando_de_listado`) nunca llevó `--jobs`. Lo verifiqué.
- El test `mutacion_con_carpeta_fija_el_listado_no_lleva_jobs` pasa desde el inicio. Es una guarda, no prueba un defecto.

## 3. Sin probar

- No corrí podman. El orquestador repite el reparto 3 con el arreglo.
- Sin probar: que cargo-mutants 27.1.0 acepte el comando y que `--in-place` corra un solo trabajo.

## 4. Preguntas para el planner

Ninguna.
