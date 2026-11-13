# Informe de la Tarea 96

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (d4e6722): 3107 verdes, 0 fallas. El número sale de restar los 8 tests nuevos al final. Esta cifra no se midió en la base.
- Final: 3115 verdes, 0 fallas, 4 ignorados.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `cmd_nextest` salía con 1 si el `cp` fallaba y copiaba junits viejos | `nextest_borra_el_junit_viejo_antes_de_correr_cargo`, `nextest_no_corta_con_el_cp_y_conserva_el_codigo`, `nextest_cargo_que_falla_sin_junit_sale_con_su_codigo`. Guarda: `nextest_copia_el_junit_y_sale_0_si_cargo_pasa` | 045c6ff |
| 2 | Nextest con 100 y sin junit caía en infraestructura | `nextest_con_codigo_100_sin_junit_da_rojo_en_los_esperados`, `nextest_con_codigo_100_sin_junit_sin_esperados_no_da_verde`. Guardas: 0 sin junit, 101 sin junit, pytest código 1 sin junit | d213bc1 |
| 3 | Frase "ninguna línea" para `modulo_borrado` y comentario de `en_cambio` | `calibra_con_bootstrap_sin_igualdad_en_el_cambio_dice_que_ningun_lugar_admite_el_defecto`, `motivo_de_defecto_de_linea_sin_candidato_dentro_del_cambio_lo_dice` | d386591 |

Cada punto lo construyó un subagente Haiku en su propio worktree (`local96-p1` a `p3`). El organizador revisó cada diff y lo aplicó con `git apply`.
El hook de commit no falló por tiempo. No hubo reintentos por ese motivo.

## 3. Qué falló o quedó incompleto

- El primer intento del commit 1 falló: `tests/ficha.rs` fijaba el comando viejo. El brief permite ese ajuste y se hizo en el mismo commit.
- El commit 3 incluye `docs/interfaces.md`, regenerado con `ROOT_ACTUALIZAR_SNAPSHOT=1`. El comentario de `en_cambio` cambió la descripción del esquema.
- Los `tarea.toml` de s2-t1 a s2-t3 y `armar_s2.py` quedan para el orquestador, como pide el brief.
- Ningún test de podman nuevo. Nada quedó sin probar.

## 4. Tests que cambiaron de valor

- `borrador_runner_nextest_usa_nextest_y_copia_junit` (`tests/ficha.rs`): fija el comando nuevo, con `rm -f` y sin `|| exit 1`.
- Dos tests de `calibra.rs` y un literal de test: fijan la frase "ningún lugar del cambio admite este defecto".
- `cmd_nextest_de_prueba` (`src/oracle/mod.rs`) copia ahora la forma nueva del comando.
- `esquema_de_cada_tipo_coincide_con_interfaces_md`: cambió `docs/interfaces.md`.

## 5. Llamadores revisados

- Ninguna firma cambió. `sin_veredicto_o_no_compila` conserva su firma y solo cambia el aviso, que ahora es el motivo recibido.
- Símbolo nuevo: `es_suite_fallida`, con un solo llamador. Constante nueva: `MOTIVO_SUITE_SIN_JUNIT`.
- `MOTIVO_FUERA_DEL_CAMBIO` cambió de valor y no de nombre.
- Se revisó con `grep -w`. No se corrió codegraph: ningún símbolo cambió de firma ni se borró.

## 6. Preguntas para el planner

1. Con 100 y junit presente, el flujo normal sigue igual. Decisión: no se cambia. El brief solo pide el caso sin junit.
2. Con nextest y 100, el junit se lee dos veces (en `correr_una` y en `juzgar_junit`). Decisión: se acepta; evita cambiar la firma de `juzgar_junit`.
