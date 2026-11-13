# Informe de la Tarea 109

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (1f60826): 3322 verdes. Es el valor final menos los 4 tests nuevos. No se corrió aparte.
- Final: 3326 verdes, 0 fallas, 4 ignorados.
- Los tests con `saltar_sin_podman` no corrieron. Los corre el orquestador al mezclar.
- Ningún test de tiempo falló en el hook. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `PlazoVencido` de la descarga iba a `Desconocido` con otro mensaje | `descarga_plazo_vencido_da_infra_con_el_mensaje_de_timeout` | 8d87816 |
| 2 | Faltaban tests que aíslan la regla del 20 % y el limpio vencido en Diseño | Ninguno. Los 3 son guardas | 27b0ab3 |

Guardas del punto 2:
- `calibra_regla_del_20_por_ciento_dos_de_diez_calibra`.
- `calibra_regla_del_20_por_ciento_tres_de_diez_no_calibra`.
- `calibra_limpio_con_plazo_vencido_en_diseno_queda_sin_veredicto_y_no_aborta`.

Cada guarda se probó con una mutación temporal de producción. Cambiar `<=` por `<` puso en rojo el test de 2 de 10. Cambiar el 20 por 30 puso en rojo el de 3 de 10. Anular la rama `PlazoVencido` puso en rojo el de Diseño. La producción se restauró.

## 3. Qué falló o quedó incompleto

- Nada quedó sin hacer.
- Los 18 tests de descarga que necesitan podman no corrieron aquí.

## 4. Lectores revisados

| Lugar | Decisión |
|---|---|
| `descarga.rs` `ejecutar_descarga` | Cambia. `PlazoVencido` da `Diagnostico::Infra("timeout tras N s; contenedor X eliminado")`. |
| `descarga.rs` `bajar_imagen` | Sin cambio. Recibe `Infra` de `correr_podman_acotado`, que usa `esperar` y nunca da `PlazoVencido`. |
| `contenedor.rs` `infra_de` (test) | Sin cambio. Lee `error_de_infra`, no `ejecutar`. |
| `perfiles/herramientas.rs` | Sin cambio. Propaga con `?`. |
| `configs.rs` y `mutacion.rs` | Sin cambio. Usan `e.to_string()`, y el texto es igual. |
| `oracle/mod.rs` `correr_en_podman` | Sin cambio. Ya distingue `PlazoVencido` desde la Tarea 108. |
| `entrega.rs` y `corre/ejecutor.rs` | Sin cambio. Reciben `Infra` de `correr_comando` y `esperar`. |
| `tests/descarga.rs`, `tests/contenedor.rs` | Sin cambio. Ningún test espera el texto viejo por un plazo. |

Codegraph y `grep -w` coinciden en los llamadores de `contenedor::ejecutar`.

## 5. Tests que cambiaron de valor

Ninguno cambió de valor. El helper `medir_con_error` recibe ahora la dimensión. Sus tres llamadores pasan `Estructura`.

## 6. Preguntas para el planner

1. El brief pide un "juez falso" en Diseño. El test usa el evaluador falso de `medir_con_error`, no un `JuezFalso`. Decisión: el evaluador falso basta, porque el plazo vence antes del juez. Falta confirmar.
2. `descarga.rs` tiene una costura nueva, `ejecutar_descarga_con`, para inyectar el ejecutor. Decisión: se acepta, porque es la forma más pequeña de probar sin podman.
3. Los dos tests del 20 % duplican en parte el test existente `calibra_limpios_sin_veredicto_hasta_el_20_por_ciento_calibra`. Decisión: se dejan, porque aíslan cada caso con su nombre.
