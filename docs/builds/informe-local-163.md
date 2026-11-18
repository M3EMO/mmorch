# Informe de la Tarea 163

## 1. Resultado de la suite

- Base (93b6d9a): no medí un conteo propio. El hook de commit pasó en los puntos 2 y 3 sin fallas.
- Final, con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`: 3989 verdes, 0 fallas, 5 ignorados.
- El hook de commit no falló por tiempo. No hubo reintentos.
- La suite con podman queda para el orquestador.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 2 | `preparar_rust` no recibía el ejecutor de `cargo vendor` | `descarga_preparar_rust_con_no_matching_package_da_el_corte_no_instala` (no compilaba) | a3c9ecf |
| 3 | El informe de un corte no decía que las suites no corrieron | `informe_de_un_corte_no_instala_dice_que_las_suites_no_corrieron` (no compilaba). Guarda: `informe_normal_no_dice_que_las_suites_no_corrieron` | a0f834e |
| 1 | "No instala" y "lock inválido" daban `SinDecision` | `oraculo_corte_lock_invalido_trae_informe_que_nombra_el_hallazgo` (SinDecision, esperaba NoListo). El test del punto 2 pasó a exigir el tipo `NoInstala` (daba `Otro`). | 63f6812 |

Guardas del punto 1: `corte_no_instala_da_no_listo_rojo_y_nombra_el_corte`, `corte_fuente_no_aceptada_da_sin_decision_con_informe`, `corte_con_secreto_de_severidad_3_da_bloqueado` y `cortes_dejan_las_nueve_dimensiones_sin_veredicto`. Estos cuatro pasaron en la primera corrida porque prueban `veredicto_de_corte` con el tipo dado. El rojo real del cableado salió de los dos tests de arriba.

## 3. Qué falló o quedó incompleto

- El corte "no instala" no se prueba de punta a punta con `evaluar_falso`: necesita un contenedor. Lo prueban `preparar_rust` con ejecutor falso y `veredicto_de_corte`.
- Falta la verificación de b8d0db4058 con su lock real. La corre el orquestador.
- No corrí `calibra.rs`. `rompe_la_compilacion` lee `no_compila`. Un corte ahora puede marcarlo. No verifiqué si afecta una calibración.

## 4. La decisión de cada corte

| Corte | `no_compila` | Decisión | Estado | Test |
|---|---|---|---|---|
| No instala | sí | NoListo | Rojo | `corte_no_instala_da_no_listo_rojo_y_nombra_el_corte` |
| Lock inválido | sí | NoListo | Rojo | `oraculo_corte_lock_invalido_trae_informe_que_nombra_el_hallazgo` |
| Fuente no aceptada | no | SinDecision | SinVeredicto | `corte_fuente_no_aceptada_da_sin_decision_con_informe` |
| Cualquiera, con secreto de severidad 3 | según el corte | Bloqueado | | `corte_con_secreto_de_severidad_3_da_bloqueado` |

Las nueve dimensiones quedan sin veredicto en los tres cortes.

## 5. Tests que cambiaron de valor

- `oraculo_corte_lock_invalido_trae_informe_que_nombra_el_hallazgo`: de SinDecision/SinVeredicto a NoListo/Rojo. Un lock que no parsea no compila.
- `assert_corte_con_informe`: recibe la decisión y el estado esperados. Ahora busca "Las suites no corrieron" en el resumen, no el detalle del hallazgo. El detalle sigue en los riesgos.
- `oraculo_descarga_sin_veredicto_deja_las_nueve_sin_veredicto` pasó a `oraculo_corte_fuente_no_aceptada_deja_las_nueve_sin_veredicto`. Prueba "fuente no aceptada".
- Tests de otros archivos, solo por la firma nueva `SinVeredicto(_, corte)`: `src/oracle/descarga/python.rs` (4), `tests/entrega.rs` (1), `tests/descarga.rs` (6).

## 6. Llamadores revisados

- `preparar_rust` (descarga.rs): codegraph y `grep -w` dan un llamador, `preparar_con_juez`. La función homónima de `colab.rs` es otra y no cambió.
- `descargar_vendor`: un llamador, `preparar_rust`.
- `Preparacion::SinVeredicto`: llamadores en `descarga.rs`, `mod.rs`, `python.rs` (tests) y `tests/descarga.rs`, `tests/entrega.rs`. Todos al día.
- `motivo_de_la_descarga`: se borró. Cero usos.
- `corte` (2 argumentos) sigue igual para `python.rs`. `corte_de` es nuevo y lo usa `preparar_rust`.

## 7. Preguntas para el planner

1. **Cortes de Python.** `python.rs` no está en mi lista. Sus cortes usan `corte(...)` y dan el tipo nuevo `Corte::Otro`. No marcan `no_compila`, así que siguen en SinDecision. Decisión: no cambié Python. ¿El "no instala" de Python debe dar NoListo como el de Rust?
2. **Informe de Python.** Sus cortes ahora dicen "Las suites no corrieron: la descarga se cortó (otro corte)." Es cierto, pero el tipo es vago. Decisión: lo dejé.
3. **Cortes por política.** Config que ejecuta con credencial, e imagen base que pide credenciales, usan `FuenteNoAceptada` (SinDecision). Decisión: son negativas de Root por política, como pide P121.
4. **Formato del motivo.** El motivo de las nueve dimensiones pasó de "descarga sin veredicto: <detalle>" a "descarga sin veredicto (<tipo>): <detalle>". El informe lee el tipo de ese texto, porque el punto 3 solo toca `informe.rs`. Decisión: la función `motivo_de_corte` fija el formato en un solo lugar.
