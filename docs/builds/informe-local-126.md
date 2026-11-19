# Informe de la Tarea 126

## 1. Resultado de la suite

- Base (5deb835): no la medí. El disco está casi lleno y no creé otro worktree.
- Final, con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`: 3521 verdes, 0 fallas, 5 ignorados.
- El orquestador corre la suite con podman, dos veces seguidas, al mezclar.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Cada helper escribía su ejecutable sin probarlo | Con un solo intento de sonda, la repetición con fork falló 3 de 3 con ETXTBSY (lo midió el subagente). Con el helper en `todo!`, los 3 tests fallaron | 836557c |
| 2 | Helpers de `src/` sin sonda o con sonda propia | Ninguno. Guardas: los tests de cada módulo y la repetición del punto 1 | 26b9555 |
| 3 | Helpers de `tests/` sin sonda o con sonda propia | Ninguno. Guardas: los tests de `corre`, `colab`, `entrega` y `test_seguro` | aebc792 |
| 4 | La aserción no mostraba el error de `cortar_vencido` | Ninguno. Es un cambio de texto, y queda como guarda | 062159c |

## 3. Qué falló o quedó incompleto

- No medí la base de la suite. Solo tengo el total final.
- Los dos cambios de `tests/entrega.rs` usan `saltar_sin_podman`. No corrieron contra podman.
- `src/oracle/pendientes.rs:698` no cambió. Es código de producción, fuera de la tabla.
- `src/oracle/mod.rs:677` y `src/oracle/perfiles/herramientas.rs:571` son carpetas. No cambiaron.
- El subagente del punto 2 dejó `.codegraph/` en el worktree. Git no lo muestra. Un hook bloqueó borrarlo.
- Un hook de commit no falló por tiempo. No reintenté ningún commit.

## 4. Los lugares cambiados

Todos son scripts `#!/bin/sh`. Ninguno es un binario copiado.

- `src/prueba_ejecutable.rs`: helper nuevo (`escribir_ejecutable`, `escribir_ejecutable_bytes`).
- `src/limpieza.rs`: `podman_falso` (antes línea 406).
- `src/ficha.rs`: cargo falso de `correr_nextest` (844).
- `src/corre/corrida.rs`: `podman_falso` (3822), `podman_del_orden` (4046), `podman_con_cuerpo` (4055).
- `src/corre/ejecutor.rs`: `programa_falso` (1372).
- `src/oracle/colab.rs`: colab-mudo, colab-lenta, colab-marca y el falso de stdin (2452, 2532, 2561, 3246).
- `src/oracle/pendientes.rs`: `unshare` y `python3` de `paquete_con_falsos` (1541).
- `tests/comun/ejecutable.rs`: copia del helper. Su comentario nombra el original.
- `tests/corre.rs`: `podman_falso` (251). `tests/colab.rs`: 111 y 473. `tests/entrega.rs`: 952 y 1095.
- `tests/test_seguro.rs`: el helper local usaba temporal y rename. Ahora usa el compartido.
- `tests/test_seguro.rs`: `falso`, `falso_de_lock` y el shim `sh` pasaron de `#!/usr/bin/env bash` a `#!/bin/sh`. Su cuerpo es POSIX. Sin el cambio, la sonda los correría enteros.
- `tests/test_seguro.rs`: `copia_del_script` no usa el helper. El script es bash, sin sentinela. Se lanza siempre con `bash`.
- `tests/test_seguro.rs`: el test `escribir_ejecutable_deja_solo_el_final_y_ejecutable` mantiene `#!/usr/bin/env bash`. Su `echo hola` es inofensivo en la sonda.

## 5. Tests que cambiaron de valor

- Borrados en `src/corre/corrida.rs`: `sonda_de_podman_falso_no_deja_marca` y `podman_falso_no_da_etxtbsy_con_fork_concurrente`. El helper único los cubre.
- Borrado en `src/limpieza.rs`: `sonda_de_podman_falso_no_deja_marca`. Usaba `--sonda`, que ya no existe.
- Borrados en `tests/corre.rs`: `podman_falso_sonda_sale_con_0_rapido_y_no_deja_pid` y `podman_falso_no_da_etxtbsy_con_fork_concurrente`. Mismo motivo.
- `src/limpieza.rs::podman_falso_no_da_etxtbsy_con_fork_concurrente` sigue. Prueba el helper en concurrencia.
- Tests nuevos en `src/prueba_ejecutable.rs`: tres guardas.

## 6. Llamadores revisados

- `escribir_podman_falso` en `limpieza.rs` y en `corrida.rs`: su único llamador era `podman_falso` del mismo módulo. `grep -w` y `codegraph callers` no hallaron otros. Codegraph dio "Symbol not found".
- `podman_falso` conserva el nombre. Sus llamadores no cambiaron.
- El helper local de `tests/test_seguro.rs` tenía cuatro llamadores: `falso`, `falso_de_lock`, el shim `sh` y un test. Todos usan el helper compartido.

## 7. Preguntas para el planner

1. `src/oracle/pendientes.rs:698` es código de producción con `from_mode(0o755)`. No lo toqué. ¿Una tarea aparte lo revisa? Decisión: lo dejé.
2. El helper solo pone sentinela en `#!/bin/sh` exacto. Un `#!/bin/bash` correría entero en la sonda. Decisión: no lo amplié. Hoy ningún script del alcance lo necesita, salvo el de `test_seguro` ya descrito.
3. Un error de spawn distinto de ETXTBSY entra en pánico en la sonda. Decisión: que se vea.
4. El helper de `tests/comun/` es una copia. Si el original cambia, hay que copiar a mano. Decisión: lo dejé como pide el brief.
