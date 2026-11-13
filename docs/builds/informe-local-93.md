# Informe de la Tarea 93

Rama `laptop/tarea-93`, base 1ab1ac5. Los dos puntos los construyeron subagentes Haiku en paralelo. El punto 2 usó un worktree aparte y entró por cherry-pick.

## 1. Resultado de la suite

- Base: no la medí antes de empezar. Es una omisión mía.
- Final con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`: la biblioteca da 2511 verdes, 0 fallas. Los demás binarios dan 0 fallas.
- Con podman: `tests/entrega.rs` da 52 verdes. Los tests `entrega_podman` de la biblioteca dan 3 verdes.
- Sin `ROOT_HOME` fijado, dos tests de `mcp` fallan. Piden el entorno de `test-seguro.sh`. No tienen relación con la tarea.
- El hook de commit pasó a la primera. No hubo reintentos.

## 2. El motivo de "sin `--user`"

- El commit 10a58b7 y `informe-local-91.md` no dan ningún motivo.
- El test solo fijaba la ausencia de `--user`. Esa ausencia vino con el cambio a `podman exec`.
- Con keep-id, `/salida`, la carpeta de trabajo, la caché y `/tmp` siguen escribibles. Lo comprobé con podman real.
- `/tmp` es tmpfs con modo 1777. No hizo falta ninguna opción nueva.

## 3. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El contenedor de evaluación corría como root y el `sh` externo no tenía ruta absoluta. | `argumentos_separado_deja_el_contenedor_vivo_y_sin_rm`, `argumentos_separado_no_corre_como_root_del_contenedor`, `sesion_paso_args_usa_la_ruta_absoluta_de_sh_como_programa_del_exec`, `eval_arranque_trae_userns_keep_id_y_paso_usa_sh_absoluto`, `entrega_podman_instalar_que_reemplaza_sh_falla_por_permisos` | 8d68793 |
| 2 | Una corrida lanzaba roles y vueltas sin cupo. | `corre_rol_que_deja_de_ir_por_cupo_no_se_lanza_y_la_corrida_para`, `corre_vuelta_que_deja_de_ir_por_cupo_no_se_lanza_y_la_corrida_para` | 573caa9 |

Guardas del punto 1: `argumentos_con_rm_no_llevan_userns_keep_id`, `argumentos_separado_mantiene_tmp_escribible_como_tmpfs`, `entrega_podman_keep_id_escribe_en_salida_trabajo_tmp_y_cache`, `entrega_podman_preludio_rust_sin_root_copia_el_config`, `entrega_podman_python_sh_falso_en_venv_no_pasa_escenario_que_falla` (en `python:3.12-slim`), `entrega_podman_instalar_que_reemplaza_sh_no_pasa_un_escenario_que_falla` y `entrega_podman_sh_falso_en_usr_local_bin_no_pasa_un_escenario_que_falla`.
Guarda del punto 2: los tests existentes de corridas con cupo completo.

## 4. Qué falló o quedó incompleto

- Sin podman no hay rojo para el `sh` externo absoluto. Con keep-id, el producto ya no escribe en los directorios del PATH. Solo lo cubre el unitario.
- Los guardas de Python y de `/usr/local/bin` no se corrieron contra el código viejo. Solo el de `cp /bin/true /bin/sh` se vio en rojo.
- No corrí clippy.
- El `Gate` de cupo no dice la causa si la vía pasa a API sin `SinPresupuesto`.
- El subagente del punto 1 dejó la carpeta `~/.cache/exp93-VEq5`. El hook frenó el borrado. Hay que borrarla a mano.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales.

## 5. Tests que cambiaron de valor

- `argumentos_separado_deja_el_contenedor_vivo_y_sin_rm` ahora exige `--userns=keep-id`. Sigue sin `--rm` y sin `--user`.
- Cinco tests de `paso_args` y `correr_escenario` esperaban `"sh"` como programa del exec. Ahora esperan `"/usr/bin/sh"`, la ruta que detecta la sesión.

## 6. Llamadores revisados

- `argumentos_separado`: sin cambio de firma. Lo llaman `entrega.rs:298` y `entrega.rs:2218`. Los dos son arranques de evaluación. Revisé con grep. Codegraph no se corrió.
- `Sesion::paso_args`: sin cambio de firma. Lo llaman `paso` y `lanzar`, y los tests.
- `exigir_cupo`: mantiene su firma y sus mensajes. Su único llamador es `corrida.rs:187`. Ahora delega en `modelo_por_cupo`.
- `DepsCorre` y `lib.rs` no cambiaron.

## 7. Preguntas para el planner

1. `args_arranque` arranca los contenedores de escenarios de la imagen construida (`Sesion::de_imagen`). Siguen como root, sin keep-id. El brief limita el cambio a la evaluación. Decisión: no lo toqué. Una imagen con `USER` propio puede romperse con keep-id.
2. Cualquier error de `rutear_en` en el nuevo ruteo frena la corrida, no solo `SinPresupuesto`. Decisión: frena, con el mismo motivo.
3. El nuevo ruteo va después de esperar la vigencia del login y antes de lanzar. Decisión: así no se espera el login de un rol que no corre.
4. Los roles ya terminados en una corrida reanudada no se rutean otra vez. Decisión: no se lanzan, así que no hace falta.
5. El modelo de cada lanzamiento sale del nuevo ruteo. La fila sigue con los modelos del ruteo inicial. Decisión: lo pide el punto 5 del brief.
