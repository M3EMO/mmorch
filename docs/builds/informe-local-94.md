# Informe de la Tarea 94

## 1. Resultado de la suite

- Base (informe de la Tarea 93, `ROOT_SIN_PODMAN=1`): la biblioteca da 2511 verdes y 0 fallas.
- Final (`ROOT_SIN_PODMAN=1`, `tools/test-seguro.sh`): 3089 verdes, 0 fallas, 4 ignorados en todos los binarios.
- Clippy: `cargo clippy --all-targets` sobre la punta no da avisos. El punto 3 no necesitó cambios.
- Un test de tiempo, `corre::ejecutor::tests::vencimiento_sigue_borrando_el_contenedor_por_nombre`, falló una vez en una corrida parcial del subagente. Pasó aislado y en la suite completa.
- No se corrió podman: los puntos no tocan `entrega`.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | `calibrar_medida` creaba su carpeta en `/tmp` | `calibra_carpeta_temporal_queda_dentro_de_home_tmp` | d8965d7 |
| 2 | La fila guardaba el modelo inicial. El `Gate` de cupo no daba la causa | `corre_fila_guarda_el_modelo_del_ultimo_lanzamiento_del_rol`, `gate_de_cupo_que_pasa_a_api_nombra_la_via_y_el_motivo` | 0d00524 |
| 3 | Clippy sin correr | Sin test: no hay avisos | sin commit |

Guarda del punto 1: `calibra_carpeta_temporal_se_borra_al_terminar_aunque_falle`. Estaba verde antes del cambio, porque `TempDir` ya borra su carpeta. No fue rojo.

## 3. Qué falló o quedó incompleto

- **Punto 2, lista de modelos por vuelta.** No se hizo. Pide un campo nuevo en `Fila`, que vive en `src/tipos.rs`. Ese archivo está fuera de la tabla.
- La fila guarda el modelo del último lanzamiento en el campo `modelos` que ya existe. No hay campo nuevo, así que `docs/interfaces.md` no cambió.
- Un rol reanudado como `Terminado` conserva el modelo inicial. Ningún test fija ese valor.
- Una fila vieja se lee igual: no cambió el esquema, y los tests de lectura existentes pasan.
- Punto 2, motivo del ruteo: `modelo_por_cupo` suma `motivo del ruteo: <motivo>` a su error. Eso también cambia el texto de `exigir_cupo`. Ningún test lo comparaba.

## 4. Otros `tempdir()` encontrados

- `calibra.rs`: solo el de `calibrar_medida_con_hilos`. Ya va a `home/tmp`.
- `calibrar_con`, la API de tests sin `home`, pasa `None` y conserva `tempfile::tempdir()`.
- `src/oracle/calibra/*.rs`: todos los `tempdir()` están en módulos de test.
- `compartidos.rs` usa `NamedTempFile::new_in` en la carpeta de la calibración. Ya no cae en `/tmp`.

## 5. Tests que cambiaron de valor

- `corre_rol_que_deja_de_ir_por_cupo_no_se_lanza_y_la_corrida_para` y `corre_vuelta_que_deja_de_ir_por_cupo_no_se_lanza_y_la_corrida_para`. Esperan el `Gate` con la causa y el motivo del ruteo.
- Las 17 llamadas de test a `calibrar_medida_con_hilos` ganaron el argumento `None`.

## 6. Llamadores revisados

- `calibrar_medida` y `calibrar_medida_con_hilos` ganaron `home: Option<&Path>`.
- codegraph: `calibrar_tarea_con` y `calibrar_con`.
- grep -w: las mismas dos llamadas, la definición y un comentario. La unión coincide.
- `modelo_por_cupo` cambió solo su texto. Sus llamadores son `exigir_cupo` y `rutear_por_cupo`, los dos en `corrida.rs`.

## 7. Preguntas para el planner

1. ¿Se agrega a `Fila` un campo `modelos_por_vuelta` con `#[serde(default)]`? Decisión: no se hizo, porque exige editar `src/tipos.rs` y `docs/interfaces.md`.
2. ¿`calibrar_con` debe pasar a recibir `home`? Decisión: no, es una API de tests.
