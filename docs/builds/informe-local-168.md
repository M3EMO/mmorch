# Informe de la Tarea 168

## 1. Resultado de la suite

Todas las corridas usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Corrida | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (4939980) | 4057 | 0 | 5 |
| Final (punto 3) | 4084 | 0 | 5 |

El hook de pre-commit no falló por tiempo en ningún commit. No hubo reintentos.
Los tests con podman quedan para el orquestador.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La compuerta exigía base de la laptop a las dimensiones con juez | `arquitectura_con_juez_entra_sin_base_en_la_laptop`, `diseno_con_juez_entra_aunque_la_base_difiera`, `estructura_con_juez_sin_casos_de_colab_no_entra`, `aviso_de_juez_nombra_dimension_y_tarea`, `juez_entra_sin_base_y_oraculo_sin_base_se_rechaza_con_aviso`. Guardas: Funciona sin base, Seguridad con base distinta, huella, `tarea_hash` y `ya_estaba`, las nueve huellas. | 89283a9 |
| 2 | El tope valía por trabajo, no por paquete | Python: `resto_del_tope`, `con_tope`, `decidir_tope`, `gastado_del_paquete`. Rust: solo se vio el rojo de compilación. Guardas: oráculo sin `--tope-usd`, test de la 167 con "de 30". | 33ae240 |
| 3 | `_coincide` comparaba `n`, y no había registro de trabajos hechos | `fila_de_trabajo` con otro `n`, dos trabajos con `n` distinto, `hechos`, `armar_vuelta` con `hechos.json`, `celda_3` anota el hecho, `ya_calibrado`. Guardas: regla de parada con `n=10`, tope agotado no anota hecho. | 9d463fe |

## 3. Qué falló o quedó incompleto

- **Punto 2, Rust.** El subagente agregó el campo y la lógica en el mismo paso. Los tests de comportamiento de `--tope-usd` no se vieron en rojo por separado. Hoy pasan.
- **Ventana de corte.** Si la sesión se corta entre la fila y `anotar_hecho`, el trabajo corre otra vez al retomar. Gasta API una vez más.
- **Trabajo con fallo.** Un trabajo con código distinto de 0 que deja fila se anota como hecho.
- No corrí `corre`, `calibrar`, la validez ni `colab importar` real.

## 4. La compuerta

**Dimensión con juez** (`tiene_items_de_juez`). Revisa, en orden:
1. la huella;
2. el `tarea_hash`;
3. "ya estaba en la laptop";
4. los casos de Colab: la fila trae sus `.casos.jsonl` y cada uno está en el paquete. Si no: "faltan los casos de Colab".

No exige base de la laptop y no compara `valor_base`. Cada fila que entra suma un aviso al resumen.

**Dimensión de oráculo.** Sin cambios: las tres primeras revisiones, más la base y sus motivos de hoy.

## 5. Tests que cambiaron de valor

- `linea_gasto_s2_trae_las_cinco_cifras` (`src/colab_calibra.rs`): la línea ahora termina en "; tope del paquete: X USD". El test usa `starts_with` con el texto de hoy hasta "de 30" y `ends_with` con el tope.
- `claves_fd_juez_ya_calibrado_se_salta_solo_con_fila_nueva` (`tests/claves_fd.rs`): pasó a `..._se_salta_solo_con_hecho_y_fila`. `n` ya no se compara, y `ya_calibrado` lee `hechos`.
- Tests de `src/colab_calibra.rs` y `src/lib.rs`: sumé los campos `casos_de_colab_completos` y `tope_usd` a los literales de `Entrada`, `Paquete` y `PedidoEmpaquetar`. Solo tests.
- **Otros archivos fuera de la tabla.** `tests/claves_fd.rs` guarda los tests de Python del notebook, no `tests/colab_preparar.rs`. `docs/interfaces.md` lleva la opción `--tope-usd`, como pide el brief. `src/esquema.rs` no lista opciones: no lo toqué.

## 6. Llamadores revisados

No usé `codegraph`: busqué con `grep -w`.
- `Entrada`: un literal en `importar_desde` y los de tests.
- `linea_gasto_s2`: `empaquetar` y un test.
- `PedidoEmpaquetar` y `Paquete`: `empaquetar_cli_con`, `src/lib.rs` y los literales de tests.
- `ya_calibrado`, `fila_de_trabajo`, `_coincide`: solo `celda_3` y los tests de `tests/claves_fd.rs`.

## 7. Preguntas para el planner

1. **Nombre de la dimensión en el aviso.** El nombre con mayúscula solo existe en `src/oracle/informe.rs` (privado). El aviso usa la clave: "arquitectura de s2-t1: ...". Decisión: dejar la clave y no tocar `src/oracle/`.
2. **`--tope-usd` sin `--juez`.** Decisión: error de uso "--tope-usd exige --juez". El brief no lo dice.
3. **Fuente del gasto en el notebook.** El brief dice `gasto.jsonl`, pero ese archivo se arma solo al final. El notebook suma las llamadas de `eventos.jsonl` (rotados y actual), la misma fuente y regla de `gasto.jsonl`.
4. **Trabajo con fallo y fila.** Hoy queda como hecho. ¿Debe anotarse solo con código 0?
5. **Dimensión de oráculo en `decidir_fila`.** Con `base = None` y casos presentes no cambia nada. Los avisos de juez salen después del aviso de mutantes.
