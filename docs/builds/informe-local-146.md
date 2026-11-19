# Informe de la Tarea 146

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1`. El hook de commit corre la suite completa.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (9a6a10a, final de la 145) | 3757 | 0 | 5 |
| Final (a367cf4) | 3793 | 0 | 5 |

- Los 3757 de la base vienen del informe de la 145. No medí la base de nuevo.
- Ningún test de tiempo falló en el hook. No hubo reintentos.
- Los tests con podman quedan para el orquestador.

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | Un plazo vencido en modo bwrap decía "contenedor eliminado". | `error_de_plazo_de_podman_dice_contenedor`, `error_de_plazo_de_bwrap_dice_proceso_bwrap_y_no_contenedor`, `descarga_plazo_vencido_de_bwrap_nombra_el_proceso`. El rojo fue de compilación: faltaba el campo `motor`. | ba58f64 |
| 2 | `colab empaquetar` no tenía `--n` ni `--semilla`. | `colab_empaquetar_parsea_n_y_semilla`, `colab_empaquetar_pasa_n_y_semilla_a_la_funcion`. El rojo fue de compilación. | a367cf4 |

- Guardas del punto 1: `plazo_vencido_con_su_mensaje_de_siempre_y_codigo_3` y `descarga_plazo_vencido_da_infra_con_el_mensaje_de_timeout`.
- Guardas del punto 2: `colab_empaquetar_sin_banderas_usa_defectos` y `colab_empaquetar_con_defectos_pasa_4_y_42`.
- No logré un rojo por texto en el punto 1. Cambiar la firma obliga a compilar primero.

## 3. Qué falló o quedó incompleto

- Todo el alcance está hecho.
- `empaquetar_cli` queda sin uso directo. Vive en `src/colab_calibra.rs`, un archivo de la 147.
- Para evitar el error de código muerto, `colab_empaquetar` en `src/lib.rs` llama a `empaquetar_cli` con los valores por defecto.
- Con otros valores, llama a `empaquetar_cli_con`. La pregunta 2 pide decidir si se borra `empaquetar_cli`.
- Se actualizó `docs/interfaces.md` con las banderas nuevas. `r00t esquema` no lista banderas de colab.
- La prueba de la 141 en Colab con `--n 4 --semilla 42` no se corrió. Es del orquestador.

## 4. Los lugares de `PlazoVencido`

| Lugar | Cambió |
|---|---|
| `src/oracle/mod.rs` definición y `#[error]` | Sí: suma `motor` |
| `src/oracle/mod.rs` `matches!(.., { .. })` (línea 2605 y tests) | No |
| `src/oracle/mod.rs` tres constructores de tests | Sí: suman `motor: "contenedor"` |
| `src/oracle/contenedor.rs` `error_de_plazo` | Sí: recibe el motor y llena el campo |
| `src/oracle/contenedor.rs` `ejecutar_bwrap` | Sí: pasa el pid del proceso como nombre |
| `src/oracle/contenedor.rs` test de `error_de_plazo` | Sí: usa `..` |
| `src/oracle/contenedor.rs` `{ .. }` en tests | No |
| `src/oracle/descarga.rs` mapeo a `Diagnostico::Infra` | Sí: usa `{motor}` |
| `src/oracle/descarga.rs` test existente | Sí: suma `motor` |
| `src/oracle/calibra.rs:1214` y `:8771` | No |
| `src/oracle/calibra.rs:8613` constructor de test | Sí: suma `motor` |
| `tests/contenedor.rs` | Sí: el `matches!` suma `..` |

## 5. Tests que cambiaron de valor

- Ningún test cambió de valor esperado. El texto de podman es el mismo, letra por letra.
- Tests de otros archivos que edité, solo por la firma:
  - `src/oracle/calibra.rs`, `plazo_vencido`: suma `motor: "contenedor"`. Es un archivo de la 147: la mezcla puede dar un conflicto de una línea.
  - `tests/contenedor.rs`: el `matches!` suma `..`. Es un test de podman.

## 6. Llamadores revisados

- `error_de_plazo`: dos llamadores. Los dos están en `contenedor.rs` y los dos cambiaron.
- `PlazoVencido`: ver la sección 4. Cubre `grep -w` en `src` y `tests`.
- `colab_empaquetar`: un llamador en `src/lib.rs`, que cambió.
- `ColabAccion::Empaquetar`: tres lugares, a saber `cli.rs:448` con `{ .. }`, `lib.rs` y los tests nuevos.
- `empaquetar_cli`: el único llamador es `colab_empaquetar`.
- No corrí `codegraph`. El índice no existe en este worktree y `grep -w` dio resultados completos. Un cero de uno solo no prueba nada.

## 7. Preguntas para el planner

1. **Nombre en modo bwrap.** El texto dice "proceso bwrap <pid> eliminado". Usé el pid del proceso como nombre, porque "proceso bwrap bwrap" repetía la palabra. Decisión: pid.
2. **`empaquetar_cli` sin uso.** La función vive en un archivo de la 147. Decisión: mantenerla viva desde `lib.rs`. Pregunta: ¿se borra al mezclar con la 147?
3. **Campo `motor`.** Es `&'static str`, no un enum. Decisión: texto fijo, porque solo existen dos motores.
