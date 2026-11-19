# Informe de la Tarea 114

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh --no-fail-fast`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base 04cf8e7 | 3354 | 0 | 4 |
| Final (6eb4260) | 3365 | 0 | 4 |

- Los tests con podman quedan saltados. El orquestador los corre al mezclar.
- El hook frenó el primer intento del commit del punto 2 por 3 tests de `mod.rs`. No fue un test de tiempo.
- Corregí los 3 tests y el commit pasó al segundo intento.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La evaluación leía la calibración desde `root_home()` | `oraculo_calibra_con_el_tarea_toml_del_home_de_la_evaluacion`, `calibra_pruebas_no_leen_root_home_implicito` | 9e88611 |
| 2 | `inestables_de` aceptaba filas sin huella o sin `tarea_hash` | `inestables_de_sin_tarea_hash_no_los_da`, `inestables_de_sin_huella_en_dimension_sin_juez_no_los_da` | bbbff02 |
| 3 | Un `no_aplica` con valor 0 pasaba como medido | `motivo_de_deteccion_no_aplica_con_valor_cero_no_detecta`, `global_no_cuenta_el_puntaje_con_no_aplica_aunque_este_calibrado`, `motivo_con_juez_no_aplica_no_detecta_aunque_el_item_baje`, `mutante_vivo_no_aplica_no_detecta_aunque_el_hallazgo_coincida`, `detecta_item_no_aplica_no_detecta_aunque_el_nivel_baje`, `item_del_defecto_con_no_aplica_no_detecta`, `completitud_de_no_aplica_da_none` | 6eb4260 |

Guarda del punto 1: el test `calibra_pruebas_no_leen_root_home_implicito` lee el código fuente del módulo de tests. Busca `root_home()`, `.aplicar(` y `.motivo_sin_calibrar(`. Las cadenas van partidas con `concat!`.

## 3. Qué falló o quedó incompleto

- Todos los puntos están hechos y probados sin podman.
- El test unitario del punto 1 vive en `src/oracle/mod.rs`, no en `calibra.rs`. Sus helpers (`tarea_rust`, `repo_rust`, `correr_suites`) viven en los tests de `mod.rs`.
- No corrí la validez, `corre`, `calibrar` ni la pasada reales, como pide el brief.

## 4. Lectores revisados

| Lugar | Decisión |
|---|---|
| `global_de`, `calibra.rs` | Cambia a `!sin_valor()`. |
| `motivo_con_juez` (era `:2736`) | Cambia a `sin_valor`. |
| `motivo_de_deteccion` (era `:2754`) | Cambia a `sin_valor()`. |
| `motivo_de_mutante_vivo` (era `:2787`) | Cambia a `sin_valor()`. |
| `detecta_item` (era `:2841`) | Cambia a `sin_valor()`. |
| `regla.rs:43`, `detecta_con_juez` | Cambia a `sin_valor()`. |
| `completitud_de`, `validez.rs:546` | Cambia a `!sin_valor()`. El llamador de `validez.rs:1063` hereda el cambio. |
| `compartidos.rs:88` y `:90` | Sigue en `sin_veredicto`. Necesita el motivo `NO_MEDIDA` y `FALLA_DEL_MODELO`. |
| `motivo_de_la_base` y `motivo_sin_veredicto` | Siguen. Copian el motivo, no preguntan por valor. |
| `motivos_de_construccion` | Sigue. Lee el motivo de construcción. |
| `Caso.sin_veredicto` (`:1807`, `:1812`, `:1838`, `:2871`, y los de los tests) | Siguen. Leen otro tipo. Salen de `motivo_sin_veredicto`. |

Los números de línea del brief estaban corridos. Las funciones son las de la tabla.

## 5. Tests que cambiaron de valor

- `inestables_de_da_los_de_la_fila_mas_reciente`: sus filas tenían `tarea_hash: None`. Ahora llevan `Some("tarea-actual")`. Fijaba la aceptación de filas viejas.
- `oraculo_diseno_excluye_el_item_inestable_del_puntaje`, `oraculo_diseno_cobertura_descuenta_el_peso_del_item_inestable` y `oraculo_diseno_con_estables_de_menos_de_la_mitad_queda_inestable` (`mod.rs`): cambiaron por el helper `calibracion_con_huella`. La fila ahora lleva el `tarea_hash` de `tarea_rust`.
- Tests de otros archivos que edité (solo tests):
  - `src/oracle/mod.rs`: el helper `calibracion_con_huella`, y un test nuevo.
  - `tests/calibra.rs:990`: pasa a `aplicar_en(home.path(), ...)`.
  - 13 tests de `calibra.rs` pasan a `aplicar_en` y `motivo_sin_calibrar_en` con una carpeta temporal.

## 6. Llamadores revisados

| Símbolo | Codegraph | grep -w |
|---|---|---|
| `aplicar` (borrado) | `evaluar_con`, 2 tests de `calibra.rs` | `mod.rs:909`, ~20 tests de `calibra.rs`, `tests/calibra.rs:990`. `inyectar.rs` tiene otra función `aplicar`, sin tocar. |
| `motivo_sin_calibrar` (borrado) | `evaluar_con` | `mod.rs:921` y 6 tests de `calibra.rs`. Los demás hits son el campo de `Puntaje`. |
| `huella_vigente` (borrado) | no corrí codegraph | Solo `inestables_de`. |
| `tarea_vigente` (queda) | no corrí codegraph | `motivo_sin_calibrar_en` la usa. |

Codegraph no vio los tests que llaman por método. La unión con grep cubrió el resto. Ningún llamador de producción fuera de `mod.rs` usaba los símbolos borrados.

## 7. Preguntas para el planner

1. **`tarea_vigente` sigue aceptando una fila sin `tarea_hash`, en `motivo_sin_calibrar_en`.** Decisión: la dejé. Solo elige el motivo `MOTIVO_OTRA_TAREA`, y el brief no la nombra. Su doc dice "Sin hash, la fila es vieja y sirve". Contrasta con la regla de la Tarea 111.
2. **Un `no_aplica` en la línea base o en un caso no se trata como sin veredicto.** Decisión: no toqué `motivo_sin_veredicto` ni `Caso.sin_veredicto`. La calibración salta Entrega con `no_aplica` antes de medir. No encontré un camino de producción que lo cree.
3. **El test unitario del punto 1 está en `mod.rs`.** Decisión: lo dejé ahí, por los helpers. Moverlo exige exponer helpers de test.
