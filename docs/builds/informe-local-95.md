# Informe de la Tarea 95

## 1. Resultado de la suite

Con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`:

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (03308b9) | 3089 | 0 | 4 |
| Final | 3107 | 0 | 4 |

Los tests con `saltar_sin_podman` no se corrieron. El orquestador los corre al mezclar.
Dos fallos del hook. El primer commit del punto 2 falló por clippy (`too_many_arguments` en un helper de tests). Se agregó `#[allow]` y el segundo intento pasó. El commit del informe falló una vez por dos tests de tiempo de `tests/proveedores`. Pasaron solos y el reintento pasó.

## 2. Códigos de nextest

- 100 es `TEST_RUN_FAILED`: uno o más tests fallaron.
- 101 es `BUILD_FAILED`: el build de los tests falló.
- Fuente: `nextest-metadata/src/exit_codes.rs` del tag `cargo-nextest-0.9.100` en GitHub. La imagen `:3` instala esa versión (`imagenes/oraculo-rust/Containerfile`).
- El archivo no está en el registro local de cargo. Se leyó con `curl` desde `raw.githubusercontent.com`.

## 3. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `comportamiento_roto`, `modulo_borrado` | `comportamiento_con_cambio_elige_solo_igualdades_dentro_del_rango`, `comportamiento_con_cambio_sin_igualdades_no_es_aplicable`, `modulo_borrado_con_cambio_elige_un_modulo_tocado`, `inyeccion_marca_en_cambio_solo_con_rangos_y_defecto_de_linea`, `calibra_con_bootstrap_los_defectos_de_linea_caen_en_el_cambio`, `calibra_con_bootstrap_sin_igualdad_en_el_cambio_dice_que_ninguna_linea_admite_el_defecto`, `motivo_de_defecto_de_linea_sin_candidato_dentro_del_cambio_lo_dice` | 964a25a |
| 2 | suite que no compila | `nextest_con_codigo_101_y_junit_verde_da_rojo_en_los_esperados`, `nextest_con_codigo_101_sin_junit_da_rojo_y_no_es_infra`, `nextest_no_compila_en_el_contrato_toma_solo_los_contrato`, `nextest_no_compila_en_la_oculta_quita_los_contrato`, `nextest_no_compila_quita_los_excluidos`, `nextest_no_compila_sin_esperados_no_da_verde` | 9435948 (cherry-pick de fae7d33) |

Guardas nacidas en verde:
- Punto 1: `sin_cambio_los_dos_defectos_de_linea_eligen_como_la_api_sin_contexto` y `caso_guardado_de_antes_de_en_cambio_se_lee_con_false`.
- Punto 2: `nextest_con_codigo_100_y_fallas_sigue_el_cruce_de_siempre`, `nextest_con_codigo_0_y_junit_verde_es_verde` y `pytest_con_codigo_101_sigue_el_cruce_de_siempre`.

`docs/interfaces.md` se regeneró: solo suma `en_cambio` al esquema de `CasoGuardado`.

## 4. Qué falló o quedó incompleto

- No se corrió la validez, `corre`, `calibrar` ni la pasada real, como pide el brief.
- Un build roto sin junit no llega al rojo en la práctica. `cmd_nextest` hace `cp ... || exit 1`, y esa salida (1) cae en infraestructura. Ver pregunta 1.
- Desvío aprobado por el orquestador: el punto 1 toca `src/oracle/calibra/defectos.rs` (un literal `cambios: None` en un módulo de tests).

## 5. Tests que cambiaron de valor

Ninguno. Los tests existentes siguen con su valor. Solo se agregó el campo `cambios` a los literales de `Contexto` y `en_cambio` a los de `Inyeccion` y `CasoGuardado`.

## 6. Llamadores revisados

Se usó `grep -w`. No había `.codegraph/` en el worktree.
- `comportamiento`: un llamador, `inyectar_con`.
- `modulo_borrado`: un llamador, `inyectar_con`.
- `motivo_sin_inyeccion` (tercer parámetro nuevo): `medir_caso` y tres tests de `calibra.rs`.
- Literales de `Contexto`: `inyectar`, `medir_caso`, `defectos.rs` y cuatro tests de `inyectar.rs`.
- Literales de `Inyeccion`: dos brazos de `inyectar_con` y cuatro tests de `calibra.rs`.
- Literales de `CasoGuardado`: `de` y `de_no_aplicable`.
- Literal de `Ronda`: uno, en `calibrar_medida_con_hilos`.
- `evaluar_suite` y `correr_una`: firmas sin cambio.

## 7. Preguntas para el planner

1. **Build roto sin junit.** `cmd_nextest` sale con 1 si falta el junit. Ese caso cae en infraestructura, no en rojo. Decisión del organizador: no se tocó `ficha.rs`. La regla vale solo con el código 101.
2. **`bootstrap/` idéntico al repo bueno.** Los rangos quedan vacíos. Los dos defectos no aplican, con el motivo "ninguna línea del cambio admite este defecto". Decisión: se mantiene, sin caer al azar.
3. **Archivo nuevo del commit.** Cuenta entero como cambio, así que un módulo nuevo es candidato a `modulo_borrado`. Decisión: se sigue `rangos_cambiados` tal como está.
4. **Alcance de `en_cambio`.** Solo marca `comportamiento_roto` y `modulo_borrado`, como pide el punto 1. `assert_borrado` y los demás defectos no lo marcan. Decisión: no se amplió.
5. **Motivo con "línea" para un módulo.** `modulo_borrado` no tiene línea, pero usa la frase del brief. Decisión: se mantiene.
6. **Build roto en una repetición posterior.** Si el build falla en una repetición que no es la primera, toda la suite queda en rojo, aunque las corridas previas pasaron. Decisión: se acepta.
