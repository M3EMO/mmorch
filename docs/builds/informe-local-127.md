# Informe de la Tarea 127

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (5deb835), solo la lib | 2946 | 0 | 1 |
| Final (7d89cf4), suma de todos los binarios | 3536 | 0 | 5 |

- El hook de pre-commit corrió la suite en cada commit. No falló ningún test de tiempo.
- Un primer intento del commit 1 falló por `dead_code` (`-D warnings`). El commit 1 lleva `#[expect(dead_code)]` y el commit 2 lo quita.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Cada corrida de mutación usa un `/salida` nuevo y compila desde cero | `mutacion_con_carpeta_fija_listado_y_corrida_reciben_esa_carpeta`, `..._la_corrida_limpia_y_usa_in_place`, `..._el_listado_limpia_y_usa_in_place`, `..._borra_los_reportes_antes_de_cargo_mutants`, `mutacion_con_carpeta_de_mutacion_restaura_tras_f_y_tras_un_panico`. Guarda: `mutacion_sin_carpeta_fija_los_comandos_son_los_de_hoy` | f879834 |
| 2 | La calibración no da carpeta a la mutación | `calibra_pruebas_la_base_y_los_casos_reciben_la_misma_carpeta_de_mutacion`, `calibra_pruebas_borra_la_carpeta_de_mutacion_al_terminar`, `calibra_pruebas_no_deja_la_carpeta_de_mutacion_tras_un_error_de_caso`, `guardia_de_mutacion_borra_su_carpeta_al_soltarse_y_deja_la_raiz`. Guardas: Completitud no crea la carpeta, guardia fuera de Pruebas, guardia con pánico | 7d89cf4 |

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- Los tests `calibra_pruebas_borra_..._al_terminar` y `..._tras_un_error_de_caso` pasarían también sin el guardia, porque la temporal entera se borra al salir. Los dos tests `guardia_de_mutacion_*` prueban el guardia de verdad.
- Sin probar, por la regla de podman: que `--in-place` dé los mismos mutantes y estados, la ganancia de tiempo y que la carpeta desaparezca de `home/tmp`. Los corre el orquestador.

## 4. El comando nuevo

`PRELUDIO` es `export CARGO_HOME=/tmp/cargo-home && mkdir -p "$CARGO_HOME" && cp /cache/config.toml "$CARGO_HOME/config.toml" && `.

Listado:

```
{PRELUDIO}rm -f /salida/outcomes.json /salida/tests-fallidos.txt /salida/listado.json && rm -rf /salida/work /salida/mutants && cp -a /repo /salida/work && cd /salida/work && cargo mutants --list --json --no-shuffle --in-place --exclude-re 'delete field .* from struct .* expression' > /salida/listado.json
```

Corrida (el `-F` del filtro va antes de ` -- --offline`):

```
{PRELUDIO}rm -f /salida/outcomes.json /salida/tests-fallidos.txt /salida/listado.json && rm -rf /salida/work /salida/mutants && cp -a /repo /salida/work && cd /salida/work && CARGO_TARGET_DIR=/salida/target timeout 1200 cargo mutants --no-shuffle --in-place --jobs 1 --exclude-re 'delete field .* from struct .* expression' --output /salida/mutants -- --offline; (cd /salida/mutants/mutants.out && grep -H '^test .* \\.\\.\\. FAILED$' log/*.log) > /salida/tests-fallidos.txt 2>/dev/null; cp /salida/mutants/mutants.out/outcomes.json /salida/outcomes.json 2>/dev/null
```

- La corrida con plazo fijo agrega ` --timeout N` después de `--jobs 1`.
- Sin carpeta fija, los dos comandos son idénticos a los de antes.
- El borrado recursivo corre dentro del contenedor. Un comentario lo dice y nombra `src/borrado.rs`.

## 5. Tests que cambiaron de valor

- Ninguno. Ningún test previo fijaba el comando viejo.
- Ningún test de otro archivo se editó.
- `mutacion.rs` suma `carpeta_de_mutacion_actual()` bajo `#[cfg(test)]`, para los tests de calibración.

## 6. Llamadores revisados

No cambió ninguna firma existente. Se suman `con_carpeta_de_mutacion` y un campo en `Ronda`.

- `comando_de_listado` y `comando_de_corrida`: la firma no cambia. Leen un valor de hilo nuevo. Grep -w: llamadores solo en `mutacion.rs` y `construir_comando` (tests).
- `Ronda`: un solo literal, en `calibrar_medida_con_hilos`.
- No corrí `codegraph`: ninguna firma cambió y el grep -w cubre el campo nuevo.

## 7. Preguntas para el planner

1. **Dónde vive la carpeta.** El brief dice "dentro de `home/tmp`, con el helper de temporales". La temporal de la medición ya está ahí. Decisión: la carpeta es la subcarpeta `mutacion` de esa temporal, con guardia propio que borra por `src/borrado.rs`.
2. **Archivos fuera de la tabla.** El punto 2 tocó `mutacion.rs` para quitar un `expect(dead_code)` y sumar un getter de test. Decisión: se aceptó porque el hook falla con código muerto.
3. **Borrado extra de `/salida/mutants`.** El brief pide borrar `/salida/work` y los reportes. Decisión: se borra también `/salida/mutants`, para que no queden resultados viejos.
