# Informe de la Tarea 88 (builder local)

Rama `laptop/tarea-88`, base dc4dfc7. Cada punto lo construyó un subagente Haiku en un worktree propio. El organizador revisó cada diff antes del commit.

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (informe de la Tarea 87): biblioteca 2386 verdes, 0 fallas, 0 ignorados. Todos los binarios: 2957 verdes, 0 fallas, 4 ignorados.
- Final: biblioteca 2403 verdes, 0 fallas, 0 ignorados. Todos los binarios: 2974 verdes, 0 fallas, 4 ignorados.
- La suite con podman la corre el orquestador al mezclar.
- Ningún test de tiempo falló en el hook. No hubo reintentos de commit.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | El error de `try_wait` no borraba el contenedor por nombre. El `rm` del vencimiento no tenía plazo. | `error_de_la_espera_pide_borrar_el_contenedor_por_nombre`; `rm_por_nombre_que_vence_su_plazo_se_informa` (rojo con un stub). Guarda: `vencimiento_sigue_borrando_el_contenedor_por_nombre`. | ae9e81e |
| 2 | Un pánico en la limpieza de la señal mataba el hilo sin salir. | `panico_en_la_limpieza_igual_sale_una_vez_con_130`; `panico_en_la_limpieza_quita_la_copia_de_la_credencial`. Guarda: los tests de señal de las Tareas 84 a 87. | 54b1240 |
| 3 | Arquitectura no tenía capas reales que medir. | 10 tests de `src/oracle/capas.rs` (cadena a-b-c, ciclo b-c, aristas a la raíz, repo sin fuentes, línea TOML, verificación propia, ciclo, capa inexistente, una sola capa, uso hacia capa anterior). Guarda: 130 tests de `perfiles`. | c0d5544 |

## 3. Qué falló o quedó incompleto

- Punto 1: la guarda del vencimiento se escribió después del arreglo. No corrió en rojo, porque faltaba una costura.
- Punto 1: el `rm` también corre en la rama "el hijo fue terminado por una señal". Si ese `rm` falla, el mensaje dice "no se borró" aunque la señal ya lo borró. No es peligroso, pero confunde.
- Punto 1: el fallo de `correr_paso_de_quitar` no nombra el contenedor. Los mensajes de `cortar_vencido` y `error_con_borrado` sí lo nombran.
- Punto 3: `codegraph` no devolvió llamadores en el worktree. Se usó solo `grep -w`.
- Punto 3: `FALTA_CAPA` en `capas.rs` repite el texto de `capas_sin_modulo`. Si uno cambia, el otro debe cambiar.
- No se corrieron la validez, `corre`, `calibrar` ni la pasada reales.

## 4. Tests que cambiaron de valor

Ninguno.

## 5. Llamadores revisados

- `correr_registrado` ganó el parámetro `contenedor: Option<&str>`. Llamadores por grep: `lanzar` pasa `Some(&nombre)`; `correr_hijo` pasa `None`. Los demás llegan por `correr_hijo`.
- `fuentes_o_sin_veredicto`, `aristas_de_capas` y `alcanzables` pasan a `pub(crate)`. Su firma no cambia. Los llamadores siguen en `rust.rs`, más `capas.rs`.
- `adyacencias` es nueva. Sale de `arquitectura_nativa` sin cambiar su comportamiento. La sonda da los mismos hallazgos.

## 6. Preguntas para el planner

1. **Carpeta de trabajo de la verificación.** Decisión: `ROOT_HOME/tmp`, que ya usan colab y descarga. `calibra` usa `/tmp`. ¿Se unifican?
2. **Qué copia la verificación.** Decisión: solo los `.rs`, sin `Cargo.toml`, porque el inyector solo toca `src/`.
3. **Ciclos en la verificación.** Decisión: la verificación ignora los hallazgos de ciclo. Los ciclos propuestos van en una sola capa, y la sonda los reporta aparte.
4. **Repo sin `.rs`.** Decisión: `proponer` da error de uso (código 2). Un repo con fuentes pero sin módulos da `capas = []`.
5. **Tarea sin `[arquitectura]`.** Decisión: la verificación falla con "menos de 2 capas: hay 0".
6. **Capas de igual altura.** Decisión: dos grupos independientes con la misma altura comparten capa, como pide el brief.
7. **`--json`.** Decisión: se usa el `--json` global de la CLI, para proponer y para verificar.
