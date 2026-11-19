# Informe de la Tarea 131

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (5409dbe), solo `--lib` | 2988 | 0 | 1 |
| Punto 1 (2035669), solo `--lib` | 2993 | 0 | 1 |
| Final (751cb7c), solo `--lib` | 2999 | 0 | 1 |
| Final (751cb7c), suite completa del hook | 3574 | 0 | 5 |

- La base de 2988 sale de restar los 5 tests nuevos del punto 1 a la cifra medida tras él.
- El hook de pre-commit no tuvo fallas de tiempo. Ningún commit se reintentó.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `LastreDeMemoria` | `lastre_de_memoria_pone_la_linea_en_una_funcion_del_cambio`, `..._la_copia_del_fixture_compila`, `..._es_de_eficiencia_con_su_item`, `..._no_elige_const_fn_ni_tests`, `..._sin_cambios_cae_en_cualquier_funcion_de_produccion` (rojo por compilación) | 2035669 |
| 2 | Referencia reusada | `con_referencia_el_lado_inicial_no_recibe_llamadas`, `con_referencia_la_razon_usa_la_serie_guardada`, `referencia_con_serie_fallida_...`, `referencia_con_filtro_distinto_...`, `con_referencia_restaura_la_anterior_tambien_con_panico`, `calibra_eficiencia_la_base_mide_los_dos_lados_y_cada_caso_mide_uno` | 751cb7c |

Guardas verdes: los tests de `BucleCuadratico`, los de `eficiencia.rs` de las Tareas 115, 119 y 120, y `medir_alterna_los_lados_en_tests_y_compilacion`.

## 3. Qué falló o quedó incompleto

- No corrí nada con podman. Quedan para el orquestador los tres puntos de "Tests con podman que quedan sin probar" del brief.
- En el rojo del punto 2, cuatro tests fallaron por el `expect` de un ayudante y no por una aserción. El rojo es válido, pero no muestra la aserción.
- Ningún test usa `saltar_sin_podman`: todos corren sin podman.
- Un `cargo test --lib` directo falla 2 tests de `mcp` por falta de `ROOT_HOME`. `tools/test-seguro.sh` lo fija y pasa.

## 4. El defecto nuevo

`LastreDeMemoria` inyecta, con la sangría del cuerpo, justo debajo de la `{`:

```rust
pub fn uno(x: u32) -> u32 {
    let _lastre = std::hint::black_box(vec![1u8; 64 << 20]);
    x + 1
}
```

En el repo de prueba `repo_con_a_y_b`, la línea cae en `src/a.rs`, en `abre + 2`. La constante es `LASTRE_BYTES` en `inyectar.rs`.

## 5. Tests que cambiaron de valor

- `estado_global_es_de_diseno_con_su_item` y `efecto_en_nucleo_es_de_arquitectura_con_su_item`: `TODOS.len()` pasa de 13 a 14.
- Tests de otro archivo editados, solo el test, en `src/oracle/calibra.rs`. Ahora la rotación de Eficiencia alterna `BucleCuadratico` y `LastreDeMemoria`:
  - `calibra_camino_caliente_sin_funciones_cuenta_no_aplicables`: `(0, 5, 5)` pasa a `(2, 5, 3)`.
  - `calibra_camino_caliente_con_funcion_existente_aplica_el_bucle`: `(1, 4)` pasa a `(3, 2)`.

## 6. Llamadores revisados

No cambió ninguna firma pública existente.
- `eficiencia::medir`: misma firma. Llamador único en `src/oracle/mod.rs:841`, sin cambio.
- `medir_tests` y `medir_compilacion` son privadas y cambiaron de firma. Sus únicos llamadores están en `medir_los_dos_lados` y `medir_contra_referencia`.
- `inyectar::de_dimension` y `inyectar::inyectar` conservan su firma. Ningún `match` fuera de `inyectar.rs` es exhaustivo sobre `Defecto`.
- `codegraph` y `grep -w` coinciden en estos llamadores.

## 7. Preguntas para el planner

1. **Defecto que rompe un test.** Con la referencia fijada, un caso cuyos tests comunes difieren de los de la base deja tiempo y memoria sin valor, con el motivo "los tests comunes del caso difieren de los de la base". Una evaluación normal sí mide ese caso. Decisión tomada: aceptarlo, porque la serie guardada midió otros tests. ¿Se prefiere medir también el lado inicial en ese caso?
2. **Serie guardada con error.** El caso igual mide su repo, y el resultado lleva el motivo de la base. Gasta corridas sin necesidad. Decisión tomada: dejarlo. ¿Se saltan esas corridas?
3. **`en_cambio` de `LastreDeMemoria`.** Queda en `false`, igual que `EstadoGlobal` y `EfectoEnNucleo`. ¿Debe valer `true` cuando el lastre cae dentro del cambio?
4. **Costo de memoria.** Cada llamada a la función elegida reserva 64 MB. Decisión tomada: seguir el brief.
5. **Código repetido.** `estado_global`, `efecto_en_nucleo` y `lastre_de_memoria` repiten la inserción al inicio de una función. Decisión tomada: no unificarlas, porque sale del alcance.
6. **Costo de cada `medir`.** Cada medición de dos lados clona las series del lado inicial en un valor de hilo. El costo es pequeño y queda fuera de la ruta caliente.
