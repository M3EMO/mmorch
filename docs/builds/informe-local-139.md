# Informe de la Tarea 139: el modo bwrap

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (c038b0e): la suite de la biblioteca tenía 3052 tests verdes. Es un cálculo: 3074 menos los 22 tests nuevos.
- Final (11ed3c1): la biblioteca da 3074 verdes, 0 fallas y 1 ignorado. Los otros binarios dan verdes y 0 fallas.
- El subagente del punto 3 sumó todos los binarios: 3649 verdes, 0 fallas, 5 ignorados.
- Ningún test de tiempo falló en los hooks de commit. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | No había modo bwrap ni armado de sus argumentos | 8 tests `bwrap_*` y `backend_de_*`: no compilaban | 05c227c |
| 2 | `ejecutar` solo lanzaba podman | 5 tests `bwrap_ejecutar_*`: fallaban contra un stub | c3e89ab |
| 3 | La descarga y el digest llamaban a podman sin guarda | 4 tests: no compilaban | 11ed3c1 |

Guardas (verdes desde el inicio): `bwrap_guarda_argumentos_de_podman_siguen_igual` y `bwrap_separado_con_bwrap_da_error_y_con_podman_da_lo_de_hoy`.

## 3. Qué falló o quedó incompleto

- **El test con el `bwrap` real se saltea en esta laptop.** `--ro-bind / /` deja la raíz de solo lectura. Entonces bwrap no puede crear `/repo` ni `/salida`.
  - El error es `Can't create file /repo: Read-only file system`.
  - `--dir` no lo arregla. El test se saltea si el host no tiene `/repo` y `/salida`.
  - El runtime de Colab debe traer esas carpetas, o la Tarea 141 debe crearlas. El orquestador debe probarlo en Colab.
  - Si no existen allí, `argumentos_bwrap` necesita otro armado de la raíz (ver pregunta 1).
- **Sin guarda de modo quedan** `entrega.rs`, el builder de `corre/` y `limpieza.rs` (señales). El brief prohíbe tocarlos.
- **No probé** ni Colab ni podman.

## 4. Los argumentos de los dos modos

Una `Ejecucion` con repo `/h/r`, oráculo `/h/o`, caché `/h/c` de solo lectura, salida `/h/s`, red cerrada y `cmd` = `C`.

Podman (`argumentos`):

```
run --rm --name N --cap-drop=all --security-opt=no-new-privileges --pids-limit=512 --memory=2g
--pull=never --http-proxy=false --network none --tmpfs /tmp:rw
-v /h/r:/repo:ro,z -v /h/o:/oraculo:ro,z -v /h/c:/cache:ro,z -v /h/s:/salida:z
-e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 IMAGEN sh -c C
```

Bwrap (`argumentos_bwrap`, con `entorno` vacío):

```
--die-with-parent --unshare-user --uid 0 --gid 0 --unshare-pid --unshare-net
--ro-bind / / --dev-bind /dev /dev --ro-bind /proc /proc --tmpfs /tmp
--ro-bind /h/r /repo --ro-bind /h/o /oraculo --ro-bind /h/c /cache --bind /h/s /salida
--clearenv --setenv CARGO_PROFILE_DEV_DEBUG 0 --setenv CARGO_PROFILE_TEST_DEBUG 0
--chdir / sh -c C
```

## 5. Los lugares con podman

| Lugar | Qué hace | En modo bwrap |
|---|---|---|
| `contenedor.rs` `ejecutar_podman`, `lanzar`, `terminar`, `barrer_al_arrancar` | Corre y limpia el contenedor | No corre. `ejecutar` elige bwrap antes |
| `contenedor.rs` `correr_podman`, `lanzar_hijo` | Lanzan podman para Entrega | Sin guarda: lanzan podman |
| `contenedor.rs` `argumentos_separado` | Argumentos del contenedor vivo | Da un vector vacío (ver pregunta 3) |
| `descarga.rs` `correr_podman_acotado` | pull, exists, inspect | Da `Infra` con el motivo pedido |
| `descarga.rs:110` `PodmanReal` (clave de caché de Python) | `podman image inspect` | Sin guarda: lanza podman |
| `calibra/target.rs` `digest_de_imagen` | Id de la imagen | Da `ROOT_BWRAP_IMAGEN_ID` o `None` |
| `entrega.rs` `PodmanReal`, `ExecReal`, `FabricaReal`, `barrer_antes_de_arrancar` | Entrega: build, run -d, exec | Sin guarda |
| `entrega.rs` (códigos 125) | Solo lee códigos de salida | No lanza nada |
| `limpieza.rs` `con_vigia`, `barrer_al_arrancar` | Quita contenedores al recibir una señal y al arrancar | Sin guarda: llama a podman |
| `corre/ejecutor.rs:242`, `corre/corrida.rs:1030` | Contenedor del builder y su señal | Sin guarda |

Con `ROOT_SIN_PODMAN=1` el barrido de `limpieza.rs` no lanza podman.

## 6. Tests que cambiaron de valor

Ninguno. No edité tests de otros archivos, salvo agregar los nuevos en `descarga.rs` y `target.rs`.

## 7. Preguntas para el planner

1. **Raíz de solo lectura.** `--ro-bind / /` no deja crear los puntos de montaje que faltan. Decisión: dejé el armado del brief, porque la Prueba 0d lo validó en Colab. Si falla allí, el armado cambia a `--tmpfs /` con `--ro-bind` de `/usr` y `/etc`. ¿Se acepta ese cambio?
2. **Secretos.** El punto 6 del brief pide pasar secretos con `--setenv` y también un error si vienen. Decisión: `argumentos_bwrap` da error si hay un secreto.
3. **`argumentos_separado` da error.** Su firma devuelve `Vec<String>` y `entrega.rs` la llama. Decisión: la firma no cambia. En bwrap devuelve un vector vacío, y `argumentos_separado_con` da el error claro. ¿Se cambia la firma en una tarea que toque `entrega.rs`?
4. **Perfil de debuginfo.** Decisión: bwrap pasa `CARGO_PROFILE_*_DEBUG=0` con `--setenv`, entre el entorno de la imagen y `e.env`. Así iguala a podman.
5. **Descarga en bwrap.** El `Infra` corta la fase de descarga. Hoy un pull fallido no la corta (`descarga.rs:437`). Decisión: no lo cambié. ¿Debe ser un `Problema`?
6. **Mensajes.** `error_de_plazo` dice "contenedor bwrap eliminado". `error_de_infra` dice "podman falló" en bwrap. Decisión: no los cambié.
7. **`entorno_de_imagen` descarta claves vacías,** además de las líneas sin `=`.
