# Informe de la Tarea 89

## 1. Resultado de la suite

Todo con `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh`.

| Momento | Verdes | Fallas | Ignorados |
|---|---|---|---|
| Base (76f95c6) | 2974 | 0 | 4 |
| Final | 2990 | 0 | 4 |

## 2. Commits

| Punto | Defecto | Tests vistos en rojo | Commit |
|---|---|---|---|
| 1 | La sonda no lee imports agrupados ni `use` de varias líneas | `aristas_bloque_agrupado_llega_a_app_y_display`, `aristas_grupo_en_una_linea_da_el_primer_segmento_de_cada_camino`, `aristas_pub_crate_use_agrupado_en_varias_lineas`, `sonda_arquitectura_ciclo_escondido_en_import_agrupado_da_hallazgo`, `sonda_arquitectura_uso_hacia_capa_anterior_en_import_agrupado_da_hallazgo`, `propone_capas_con_imports_agrupados` | 6f7f1ba |
| 2 | La huella de la fila ignora el `tarea.toml` y la versión de sondas | `calibrar_con_escribe_el_hash_del_tarea_toml`, `fila_con_otro_tarea_toml_no_calibra_y_da_su_motivo`, `version_de_sondas_de_arquitectura_cambia_su_huella_y_no_la_de_seguridad` | 9c37ebf |
| 3 | El `rm` corre tras la señal y el fallo de quitar no nombra el contenedor | `hijo_terminado_por_la_senal_no_pide_otro_rm`, `error_de_quitar_con_rm_fallido_nombra_el_contenedor`, `error_de_quitar_con_ps_fallido_nombra_el_filtro` | a1061c9 |

Guardas verdes desde el inicio: `aristas_agrupado_dentro_de_test_no_cuenta`, `aristas_ignoran_self_en_grupos`, `aristas_expresion_con_crate_sigue_contando`, `fila_sin_tarea_hash_sigue_calibrando`, `dimension_solo_de_sondas_sin_huella_sigue_calibrando`.

Cada punto lo construyó un subagente Haiku en su propio worktree (`local89-p1..p3`). Después hice cherry-pick a `laptop/tarea-89`. Ningún hook de tiempo falló.

## 3. Qué falló o quedó incompleto

- **Archivos fuera de la tabla (punto 2).** Toqué `src/oracle/mod.rs` y `src/oracle/calibra/compartidos.rs`. `aplicar` y `motivo_sin_calibrar` ganaron un parámetro de hash, y `evaluar_con` los llama. `hash_del_tarea_toml` pasó a `pub(super)`. El cambio es mínimo y compila. Decidí no frenar el punto. Ver pregunta 1.
- **`inestables_de` no mira `tarea_hash`.** Una fila de otro `tarea.toml` con la misma huella aún puede excluir ítems inestables. Arreglarlo exige propagar un `Result` por `medir`. No lo hice. Ver pregunta 2.
- **Codegraph no corrió.** `codegraph.cmd` no existe en esta laptop Fedora. Usé solo `grep -w`.
- `pub(in crate::x) use ...` cuenta `x` como destino, porque el camino de la visibilidad nombra un módulo. No lo cambié.
- No corrí validez, `corre`, `calibrar` ni la pasada reales, como pide el brief.

## 4. Tests que cambiaron de valor

Ninguna aserción cambió de valor. Cambiaron las llamadas:

- Los tests de `aplicar` y `motivo_sin_calibrar` pasan el argumento de hash nuevo, también en `tests/calibra.rs`.
- Dos tests (`calibra_cinco_defectos_...`, `calibra_n_dos_...`) pasan el hash de su propia fila.
- Tres tests de huella usan `huella_con_version(d, p, version_de_sondas(d))` en vez de `huella_con_prompt`.
- Los literales de `Fila` suman `tarea_hash: None`.

## 5. Medición sobre broot

Repo `Canop_broot` en `1f77962e6ff3`, extraído con `git archive`. Comando: `r00t oraculo capas --repo <dir> --json`.

| | Antes | Después |
|---|---|---|
| Módulos | 36 | 36 |
| Aristas | 15 | 219 |
| Ciclos | 0 | 1 |
| Capas (tamaños) | 3 (3, 8, 25) | 3 (27, 2, 7) |

Antes, la sonda veía solo el 7 % de las aristas. Después, `cli` y los demás módulos de aplicación bajan a la capa superior y aparece un ciclo real.

## 6. Llamadores revisados

| Símbolo | grep -w | codegraph |
|---|---|---|
| `usos_de_crate` (borrado) | solo `aristas_de` (`rust.rs`) | no disponible |
| `aplicar` (firma nueva) | `evaluar_con` en `oracle/mod.rs`, tests de `calibra.rs` y `tests/calibra.rs` | no disponible |
| `motivo_sin_calibrar` (firma nueva) | `evaluar_con` y tests | no disponible |
| `huella_con_prompt` (reemplazada) | solo `huella_de` y tests de huella | no disponible |
| `correr_paso_de_quitar` (parámetro nuevo) | `quitar_builders_de_este_proceso` (2) y `borrar_contenedor` (1) | no disponible |
| `FALTA_CAPA` (una sola) | `rust.rs` y `capas.rs` | no disponible |

## 7. Preguntas para el planner

1. ¿Vale tocar `oracle/mod.rs` y `compartidos.rs` para pasar el hash a `aplicar`? Decisión: sí, porque no hay otra forma de comparar el hash actual. El orquestador puede rechazar el cambio.
2. ¿`inestables_de` debe validar también el `tarea_hash`? Decisión: no en esta tarea. Queda para una tarea que propague el `Result`.
3. ¿La versión de sondas debe entrar al hash en todas las dimensiones? Decisión: solo cuando es mayor que 1. Así solo Arquitectura pierde sus filas, y las otras ocho no se recalibran con gasto de API.
4. El mensaje del camino de error repite el nombre: "no se borró el contenedor X: rm del contenedor X salió con código 1". Decisión: lo dejé, porque es claro.
