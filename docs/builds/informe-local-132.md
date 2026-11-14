# Informe de la Tarea 132 (builder local)

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (9b967e3) | 3578 | 0 | 5 |
| Final | 3586 | 0 | 5 |

Ningún hook de commit falló por tiempo. No hubo reintentos.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La calibración de Eficiencia cortaba sin `[eficiencia]`, aunque la evaluación mide Rust con `bootstrap/` | `calibra_eficiencia_rust_con_bootstrap_sin_eficiencia_mide`, `calibra_eficiencia_python_sin_eficiencia_es_no_aplica_por_lenguaje`, `calibra_eficiencia_rust_sin_bootstrap_es_no_aplica_por_estado_inicial`. Guarda: `calibra_eficiencia_con_eficiencia_declarada_mide_como_siempre` | 2b8f093 |
| 2 | La rotación incluía `BucleCuadratico` en tareas sin `[eficiencia]` | `calibra_sin_eficiencia_el_bucle_no_entra_a_la_rotacion`. Los tests de lista vacía y del filtro no compilaban. Guarda: `calibra_con_eficiencia_el_bucle_entra_a_la_rotacion` | cf9eafc |

## 3. Qué falló o quedó incompleto

- Los tests de calibración de Eficiencia con Rust y `bootstrap/` (punto 1) y con `[eficiencia]` usan `if let Ok`. El veredicto falso puede dar `Err` después de llamar al evaluador. Los tests prueban que el evaluador corre. No prueban que la calibración termine con `Ok`.
- La rama de lista vacía (punto 2) no se alcanza en producción. Eficiencia siempre conserva `LastreDeMemoria`. Solo la prueba un test, con `calibrar_medida_con_defectos` y una lista vacía.
- Quedan para el orquestador: la calibración real de s2-t1, s2-t2 y s2-t3, y la subida de `memory.peak` con `LastreDeMemoria`.
- `codegraph` no se usó: el worktree no tiene `.codegraph/` y no se creó. Se usó solo `grep -w`.

## 4. La condición de Eficiencia

Función `motivo_eficiencia_no_aplica` en `src/oracle/calibra.rs`. Espeja `puntaje_de_eficiencia`.

```rust
if tarea.eficiencia.is_some() { return None; }
if tarea.lenguaje != "rust" { return Some(MOTIVO_SIN_EFICIENCIA); }
if !tarea.dir.join("bootstrap").is_dir() { return Some(MOTIVO_SIN_ESTADO_INICIAL); }
None
```

- Los dos motivos salen de `perfiles::herramientas`, sin copiar el texto.
- La fila ya no lleva el prefijo "no aplica: ". Ahora el motivo es el texto de la constante.
- El corte se aplica solo si la dimensión es Eficiencia y la función da `Some`.
- El defecto de punto 2: `filtrar_aplicables` quita `BucleCuadratico` si `tarea.eficiencia.is_none()`. Motivo de lista vacía: `MOTIVO_SIN_DEFECTOS_APLICABLES` = "ningún defecto aplica a la tarea".

## 5. Tests que cambiaron de valor

- `tests/calibra.rs`: cuatro tests leen el motivo de Eficiencia. `MOTIVO_NO_APLICA` ahora es `MOTIVO_SIN_ESTADO_INICIAL`. Motivo: el fixture es Rust sin `bootstrap/`. Tests: `eficiencia_sin_bloque_no_aplica_y_no_llama_al_evaluador`, `eficiencia_no_aplicable_queda_en_calibracion_json`, `eficiencia_no_aplicable_cli_json_imprime_la_fila_con_no_aplica`, `eficiencia_no_aplicable_cli_sale_con_0_e_imprime_el_motivo`.
- `calibra_camino_caliente_sin_funciones_cuenta_no_aplicables` y `calibra_camino_caliente_con_funcion_existente_aplica_el_bucle`: reciben `[eficiencia]` para conservar su intención. Los números esperados no cambiaron.
- `src/oracle/calibra/defectos.rs` (módulo de tests): una línea pasa la lista de defectos a `finalizar_casos`.
- 23 llamadores de `finalizar_casos` en los tests de `calibra.rs` pasan `&inyectar::de_dimension(..)`, igual que antes.

## 6. Llamadores revisados

- `MOTIVO_NO_APLICA_EFICIENCIA` (borrado): `grep -w` en `src` y `tests` da cero llamadores restantes. Codegraph: no consultado (ver sección 3).
- `finalizar_casos` (firma cambiada, parámetro `defectos`): `grep -w` da un llamador de producción en `calibra.rs`, los tests de `calibra.rs` y `defectos.rs:220`. Todos actualizados.
- `calibrar_medida_con_hilos`: firma igual. Ahora delega en `calibrar_medida_con_defectos` (nueva, privada).

## 7. Preguntas para el planner

1. Sin `[eficiencia]`, el ítem `eficiencia.tiempo` sale de los ítems medidos, porque sale `BucleCuadratico`. Decisión: se acepta, sin cambio.
2. El motivo de la fila de Eficiencia perdió el prefijo "no aplica: ". Decisión: se usan las constantes tal cual, como pide el brief. Si algún consumidor lee el prefijo, hay que revisarlo.
