# Informe de la Tarea 151

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

- Base (fb9baf6): la suite de la lib tenía 3243 verdes. El número sale de restar los 3 tests nuevos. No corrí la base por separado.
- Final: la lib da 3246 verdes, 0 fallas, 1 ignorado. Los demás binarios de test dan 0 fallas.
- El hook de commit pasó al primer intento. No hubo test de tiempo fallido.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `subir_binario` y el `cargo` mínimo subían directo, sin SHA-256 | `colab_subir_binario_chico_sube_un_trozo_y_pasa_por_la_union`, `colab_cargo_minimo_pasa_por_la_union_con_sha256`, `colab_rust_hash_malo_no_corre_rustup_y_usa_el_binario` (actualizado) | ddb9b73 |

`colab_subir_binario_con_sha256_distinto_falla_y_nombra_el_binario` también falló en rojo, porque el binario chico no pasaba por la unión. Los tests de la 150 sobre el paquete en trozos quedan como guardas y siguen verdes.

## 3. Qué falló o quedó incompleto

Nada quedó sin hacer. No corrí la suite con podman, la validez, `corre`, `calibrar` ni la pasada. Las corre el orquestador.

El permiso de ejecución no cambió de lugar. `prueba-binario.py` ya hacía `chmod` del binario y del `cargo` después de la unión. El defecto era solo la subida directa.

## 4. Las llamadas a `upload`

| Llamada | Antes | Después |
|---|---|---|
| `subir_trozo` | única vía de trozos | igual, es la única de producción |
| `subir_binario` (rama directa) | `upload` directo, sin SHA-256 | sale; la función pasó a `subir_archivo` |
| `binario_local` (`cargo` mínimo) | `upload` directo | pasa por `subir_archivo` |

`grep -n '"upload"' src/` deja una sola llamada de producción, en `subir_trozo`. Las demás líneas están en tests, y `src/colab_calibra.rs` solo tiene tests. `colab_calibra.rs:1922` llama a `subir_en_trozos`, no a `upload`.

## 5. Tests que cambiaron de valor

- `colab_rust_hash_malo_no_corre_rustup_y_usa_el_binario`: busca la unión (`exec` de `une-trozos.py`) con el destino en vez de un `upload` al destino final. El binario y el `cargo` ya no suben directo.
- Los dos tests `colab_subir_binario_*` llaman a `subir_archivo`, el nuevo nombre.
- Ningún test de otro archivo cambió.

## 6. Llamadores revisados

- `subir_binario` (renombrada a `subir_archivo`): `grep -w` y `codegraph callers` coinciden. Los llamadores eran `binario_local` y dos tests nuevos. Hoy `grep -w subir_binario` da cero.
- `subir_archivo`: la llaman `binario_local` (dos veces) y los dos tests.

## 7. Preguntas para el planner

1. Renombré `subir_binario` a `subir_archivo`, porque ya sirve para el `cargo`. Decisión: se acepta. Mantener `subir_binario` engañaba al lector.
2. La función sigue existiendo porque suma SHA-256 y unión a `subir_en_trozos`. Decisión: no la borré. Una llamada suelta repetiría esos dos pasos.
