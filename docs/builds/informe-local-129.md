# Informe de la Tarea 129: defectos para ítems del juez de Diseño y Arquitectura

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (352fd99), solo `--lib` | 2961 | 0 | 1 |
| Base, suite completa | 3536 | 0 | 5 |
| Final (cec2f00), solo `--lib` | 2974 | 0 | 1 |
| Final, suite completa | 3549 | 0 | 5 |

- La base completa sale de restar los 13 tests nuevos al final. Solo la lib cambió.
- Ningún test con podman corrió. Quedan para el orquestador.
- El hook de commit no falló por tiempo. Ningún commit se reintentó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `EstadoGlobal` | Ocho tests nuevos. No compilaban: faltaba la variante. Siete en `inyectar.rs` y `estado_global_lo_detecta_el_juez_falso` en `defectos.rs`. | 965038b |
| 2 | `EfectoEnNucleo` | `efecto_en_nucleo_pone_la_linea_en_una_funcion_del_cambio`, `efecto_en_nucleo_la_copia_del_fixture_compila` y `efecto_en_nucleo_lo_detecta_el_juez_falso`. | cec2f00 |

- Guardas del punto 2: `es_de_arquitectura_con_su_item` y `no_elige_const_fn_ni_tests` ya pasaban en rojo. Su verde no prueba la implementación sola.
- Guardas de los dos puntos: los tests de `CicloEntreCapas`, `ErrorTragado` y `EstadoGlobal` siguen verdes.

## 3. Qué falló o quedó incompleto

- Nada del brief quedó sin hacer.
- Sin probar: que los dos defectos compilen y pasen los tests de t1 dentro de la imagen. Necesita podman.
- Sin probar: que el juez real vea los defectos. Espera la repetición de P101 a y la respuesta a P107.
- Límite conocido: un archivo con `mod x {` en línea se salta entero.
- Límite conocido: una función dentro de un bloque `unsafe { }` no se detecta.
- `en_cambio` queda en `false` para los dos defectos. El lugar es un archivo del cambio, no una línea tocada.
- Las calibraciones guardadas de Diseño y Arquitectura no se revisaron. Ahora rotan dos defectos.

## 4. Los defectos nuevos

**`EstadoGlobal`** (`diseno.estado`). Ejemplo en la copia del fixture con semilla 42: `src/almacen.rs`, línea 15.

```rust
fn guardar(...) {
    let _ = ROOT_CONTADOR_CALIBRACION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ...
}

static ROOT_CONTADOR_CALIBRACION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
```

- El `static` va antes de `#[cfg(test)]`, después de la función. No mueve la línea reportada.
- Si el nombre ya existe en el archivo, usa el sufijo `_2`, `_3`.

**`EfectoEnNucleo`** (`arquitectura.adecuacion`). Ejemplo en el repo de prueba de `inyectar.rs`, con cambio sobre `src/a.rs`: línea 2 en `uno` o línea 6 en `dos`, según la semilla.

```rust
pub fn uno(x: u32) -> u32 {
    let _ = std::env::var("ROOT_EFECTO");
    x + 1
}
```

## 5. Tests que cambiaron de valor

- `estado_global_es_de_diseno_con_su_item` (`inyectar.rs`): `TODOS.len()` vale 13. Es un test nuevo de este trabajo.
- `JuezFalso` y `donde_let_guion` (`defectos.rs`): `donde_let_guion` ignora las líneas con `fetch_add` y `ROOT_EFECTO`. Si no, el contador y la lectura confundirían a `ErrorTragado`.

Tests de otro archivo que edité, solo tests, en `src/oracle/calibra.rs`:

- `calibra_un_caso_de_diseno_toma_la_mutacion_de_la_base` y `calibra_un_caso_limpio_toma_de_la_base_las_sondas_deterministas_y_corre_el_juez_y_los_tests`. La cuenta pasa de `medidos + JUICIOS_EXTRA` a `medidos + 1 + JUICIOS_EXTRA`. Con dos defectos, el caso 2 de Diseño es `EstadoGlobal`, distinto del caso 0. Antes repetía `ErrorTragado` y no llamaba al evaluador. Ahora los cuatro casos llaman.
- `calibra_arquitectura_limpio_con_item_del_defecto_que_baja_dos_detecta`. La base y el limpio traen `arquitectura.dependencias` y `arquitectura.adecuacion`. El motivo se acepta con prefijo `item_baja_2:arquitectura.`. El caso 3 sigue a `EfectoEnNucleo`, y el mock solo traía `dependencias`.

La 128 también toca `calibra.rs`. Estos tres tests pueden dar conflicto al mezclar.

## 6. Llamadores revisados

No hay `match` sobre `Defecto` fuera de `inyectar.rs` que cambiara. Los de `calibra.rs` usan comodines.

- `Defecto::dimension`, `Defecto::item` e `Inyeccion::identidad`: match exhaustivo en `inyectar.rs`. Se actualizaron.
- `de_dimension`: grep -w da `calibra.rs:1553` y `calibra.rs:2760`. Los dos toman la lista sin asumir su largo.
- codegraph no estaba iniciado en este worktree. No corrí `init` por el disco. Vale solo el grep.
- grep -w `EstadoGlobal` y `EfectoEnNucleo`: solo `inyectar.rs` y `defectos.rs`.
- `docs/interfaces.md` no nombra el enum `Defecto`. No hubo nada que regenerar.

## 7. Preguntas para el planner

1. ¿Un archivo con `mod x {` en línea se salta entero o solo esa función? Decisión: se salta entero. El `static` va en la raíz y un módulo anidado no lo ve.
2. ¿`en_cambio` debe valer `true` si la función cruza una línea tocada? Decisión: no. El brief pide "una función de un archivo del cambio". Queda `false`.
3. ¿Las calibraciones guardadas de Diseño y Arquitectura quedan viejas? Decisión: no las toqué. La rotación ahora usa dos defectos.
4. ¿Se acepta el cambio de la cuenta de llamadas en dos tests de `calibra.rs`? Decisión: sí. Es un literal que esta tarea mueve.
