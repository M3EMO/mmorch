# Informe de la Tarea 80

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh --no-fail-fast`.

- **Base (1e361b8):** no se midió por separado. Los subagentes corrieron la suite solo con su punto aplicado.
- **Punto 1 solo:** 2803 verdes, 0 fallas.
- **Punto 2 solo:** 2799 verdes, 0 fallas.
- **Final (rama `laptop/tarea-80`):** 2806 verdes, 0 fallas, 4 ignorados.
- `cargo fmt --check` y `cargo clippy --all-targets` salen limpios.
- Ningún test de tiempo falló en el hook. Ningún commit se reintentó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 (f) | Una línea oculta que el builder ve en `bootstrap/` dispara la huella | `transcript_linea_oculta_que_el_builder_ve_en_bootstrap_no_es_huella`, `transcript_nombre_oculto_que_el_builder_ve_en_bootstrap_no_es_huella`, `transcript_linea_que_el_builder_ve_en_bootstrap_no_emite` | 824b216 |
| 2 (g) | Un limpio sin veredicto da especificidad 1,0 | `calibra_un_limpio_sin_veredicto_deja_especificidad_none`, `calibra_limpio_sin_veredicto_mas_limpio_que_detecta_da_especificidad_0` | 5534852 |

- **Guardas del punto 1:** línea única sigue siendo huella, sin `bootstrap/` igual que hoy, `bootstrap/` ignora `.git` y `target`, línea única emite huella.
- **Guardas del punto 2:** `calibra_sin_casos_limpios_deja_especificidad_none`, detector perfecto y nulo.
- `calibra_limpio_sin_veredicto_mas_limpio_que_no_detecta_da_especificidad_1` falló en rojo solo por el texto. La especificidad ya daba 1,0.
- El punto 2 se construyó en el worktree `~/.cache/root-wt/local80g` (rama `laptop/tarea-80-g`) y se pasó con `cherry-pick`. El commit original es 3bd6352.

## 3. Qué falló o quedó incompleto

- El aviso "huella sin líneas distintivas" no tiene test propio. El brief no lo pidió.
- La detección offline sobre el transcript de t1 queda para el orquestador.
- La suite con podman queda para el orquestador.

## 4. Tests que cambiaron de valor (punto 2)

Los tres tienen solo limpios sin veredicto. La especificidad pasa de 1,0 a `None`.

- `calibra_limpio_sin_veredicto_no_es_falso_positivo`: la exactitud también pasa de 1,0 a `None`.
- `calibra_limpio_sin_veredicto_no_cuenta_como_detectado`.
- Un test cerca de la línea 5600 de `calibra.rs`, con el mismo motivo.

Los tests de detector perfecto y nulo no cambiaron.

## 5. Preguntas para el planner

1. **¿Dónde va el aviso de huella vacía?** El aviso es un `Gate` con fase `huella`. Va después de crear el id de la corrida, porque el `Gate` necesita ese id. Decisión: se dejó así.
2. **¿Se repite el aviso al reanudar?** Sí, se escribe otra vez. El aviso de token sí se deduplica. Decisión: sin cambio.
3. **¿Alcanza un `Gate` como señal?** Un `Gate` no invalida la corrida. Decisión: sin cambio.
4. **¿Los nombres de archivo de `bootstrap/` cuentan como texto visible?** Sí, igual que en `contrato/`. Decisión: se dejó así.
5. **¿`muestra_suficiente` debe contar solo limpios con veredicto?** Hoy cuenta `limpios` totales. Una fila puede calibrar con 1 limpio con veredicto. Decisión: sin cambio. `aplicar` sigue frenando con más de 20 % sin veredicto.
6. **¿Dónde vive el texto de especificidad?** Se creó `calibra::texto_especificidad` para poder testear el texto. `lib.rs` la llama. Es un cambio fuera de la letra del brief, dentro de los archivos permitidos.
7. **¿El formato del texto es redundante?** La línea muestra "limpios n" y "limpios con veredicto k de n". Decisión: sin cambio.
8. **¿`detecta_item` debe devolver `None` para un limpio sin veredicto?** Hoy devuelve `Some(false)`. Se filtró en `fila_de`. Decisión: sin cambio.
9. **¿Qué pasa si todos los limpios no tienen veredicto?** `exactitud_balanceada` queda `None` y la dimensión no calibra. Es coherente con la regla. Decisión: sin cambio.

## Punto 3 (respuestas 5 y 8 del planner)

### Cambio

- `muestra_suficiente` (`src/oracle/calibra.rs`) cuenta `limpios - limpios_sin_veredicto` contra `MIN_LIMPIOS`.
- Usa `saturating_sub`: una fila con más sin veredicto que limpios no alcanza el mínimo.
- Las filas guardadas no se recalculan.
- Un solo subagente Haiku construyó el cambio. Yo revisé el diff y corrí la suite.

| Parte | Tests vistos en rojo | Commit |
|---|---|---|
| `muestra_suficiente` | `muestra_suficiente_no_cuenta_limpios_sin_veredicto` (5 defectos, 5 limpios, 1 sin veredicto: no es suficiente) | ver `git log` |

- Guarda: `muestra_suficiente_con_limpios_con_veredicto_suficientes` (6 limpios, 1 sin veredicto: sí es suficiente). Pasó en rojo y en verde.
- Ningún test existente cambió de valor.

### Llamadores de `detecta_item`

- No hay índice de codegraph en este worktree (`CodeGraph not initialized`). Usé solo `grep -rnw detecta_item src tests`, como pide el brief.
- Hay cuatro llamadores. Ningún cambio de código fue necesario.

| Llamador | ¿Un limpio sin veredicto cuenta como negativo? | Resultado |
|---|---|---|
| `src/oracle/calibra.rs:1711` (llena `caso.items`) | Solo si `fila_de` lo deja pasar. `fila_de` filtra `c.sin_veredicto.is_none()` en los negativos por ítem. | Cubierto por el filtro del punto 2. |
| `src/oracle/calibra/defectos.rs:257` | No. Pasa `es_defecto = true` sobre un defecto inyectado. | `Some(false)` sin veredicto hace fallar el assert: es correcto. |
| `src/oracle/calibra/estables.rs:359` | No. Test con `es_defecto = true`. | Sin cambio. |
| `src/oracle/calibra/estables.rs:384` | No. Test con `es_defecto = true`. | Sin cambio. |

- Cobertura del filtro por ítem en `fila_de`: los tests de especificidad `calibra_un_limpio_sin_veredicto_deja_especificidad_none` y `calibra_limpio_sin_veredicto_mas_limpio_que_detecta_da_especificidad_0` miran la fila entera. No hay un test propio del ítem. No lo agregué: el planner no lo pidió.

### Suite

- `tools/test-seguro.sh` con `ROOT_SIN_PODMAN=1`: 2237 tests de la librería pasan, 0 fallan.
- `cargo test` directo falla un test de `mcp` por falta de `ROOT_HOME`. No tiene relación con este cambio.

### Preguntas para el planner

- Ninguna nueva.
