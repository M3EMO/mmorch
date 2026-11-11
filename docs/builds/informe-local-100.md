# Informe de la Tarea 100: solo los mutantes pedidos

## 1. Resultado de la suite

Todas las cifras usan `ROOT_SIN_PODMAN=1` y `tools/test-seguro.sh --no-fail-fast`.

- Base (7af9a54): 3135 verdes, 0 fallas, 4 ignorados. La cifra sale del informe de la Tarea 97.
- Final: 3144 verdes, 0 fallas, 4 ignorados. Son 9 tests nuevos.
- `cargo test` sin `test-seguro.sh` falla en 2 tests de `mcp` porque falta `ROOT_HOME`. Es un defecto del entorno, no del cambio.
- Ningún test de tiempo falló en el hook. No hubo reintento.

## 2. Commits

| Defecto | Tests vistos en rojo | Commit |
|---|---|---|
| El reporte cuenta mutantes que Root no pidió | `mutacion_fuera_de_la_muestra_no_cuenta_y_se_lista_en_orden`, `mutacion_pedido_que_falta_en_el_reporte_da_sin_veredicto`, `mutantes_muertos_no_devuelve_lo_que_no_estaba_en_la_muestra`, `pedidos_del_comando_invierte_el_filtro_exacto`, `mutacion_sonda_deja_su_filtro_y_el_mutante_de_mas_queda_fuera`, `memo_mutantes_muertos_pide_solo_la_muestra_del_filtro_grabado` | ver `git log` (Punto 1 de la Tarea 100) |

Guardas: un reporte con exactamente la muestra da el mismo puntaje que antes.
Dos tests no se vieron en rojo: `calibra_caso_guarda_el_filtro_de_su_mutacion` y `calibra_caso_viejo_sin_filtro_lee_vacio`. El subagente escribió el código antes de correrlos.

Qué hace el commit:

- `parsear_outcomes` y `mutantes_muertos` reciben los nombres pedidos.
- Un mutante de más va a `ResultadoMutacion.fuera_de_la_muestra` y no cuenta.
- Un mutante pedido que falta da Pruebas sin veredicto: "el reporte no trae el mutante pedido <nombre>". Un `Unviable` cuenta como presente.
- Cada caso de calibración guarda el `-F` en `CasoGuardado.filtro` (con `#[serde(default)]`).
- `docs/interfaces.md` se regeneró: solo suma `filtro`.

## 3. Diagnóstico de los nombres

Imagen `root-oraculo-rust:3` (cargo-mutants 27.1.0). Repo: copia de `t1-ref` de S2, con el `Cargo.lock` de broot. La caché `b2cf71cf8d5fdf07` se copió y se montó solo lectura.
La carpeta `.diag/` del worktree guarda la copia y el script `corre.sh`. No entra en ningún commit.

- Listado: 4588 mutantes. 13 son "delete field ... from struct ... expression". Todos son de tipo `StructField`.
- Los nombres del listado y los del `outcomes.json` son idénticos, carácter por carácter.
- Corrida 1: `-F` de un solo mutante "delete field" (`src/skin/purpose_mad_skin.rs:11:9`). Corrió 13 mutantes: los 13 "delete field". El mutante pedido fue uno de ellos.
- Corrida 2: `-F` de un solo mutante "replace" (`src/pattern/composite_pattern.rs:23:9`). Corrió 14 mutantes: el pedido y los mismos 13 "delete field".
- Las dos corridas dieron `total_mutants` igual a los resultados (13 y 14). Por eso la regla de "reporte truncado" no los detecta.

**Causa.** En esta versión, cargo-mutants ignora `-F` para los mutantes `StructField`. Los corre siempre. El nombre no cambia entre el listado y la corrida. El filtro `-F` exacto es correcto.
No se probó con otra versión de cargo-mutants.

Efecto con el cambio: los 13 quedan en `fuera_de_la_muestra` y no suman al puntaje. Todos salieron vivos (`MissedMutant`), así que antes bajaban el puntaje sin motivo.
Cada corrida de Pruebas paga además el tiempo de esos 13 mutantes (unos 1 s de compilación cada uno en broot).

## 4. Qué falló o quedó incompleto

- **El aviso del veredicto no está conectado.** La tarea pide "cargo-mutants corrió N mutantes que no estaban en la muestra". `mod.rs:1016` toma solo `.sonda` de `sonda_mutacion_en`. Conectar el aviso exige tocar `src/oracle/mod.rs`, que está prohibido. Los datos ya están en `ResultadoMutacion.fuera_de_la_muestra`. Quité la función `aviso_de_mas` porque clippy rechaza código sin uso. El texto del aviso va en este informe, arriba.
- `memo.rs` quedó fuera de la tabla de archivos. Hubo que cambiarlo: sin eso `assert_borrado` seguía leyendo mutantes de más. El cambio es mínimo: lee la muestra del `-F` grabado.
- `tests/mutacion.rs` cambió solo en las llamadas a `parsear_outcomes`, para compilar.
- `calibra.rs`: se agregó `filtro` a `Caso`, a `Evaluada` y a `CasoGuardado`. Nada más.
- El `-F` guardado queda vacío en un caso que no llegó a armar la corrida (sin herramienta, listado fallido).
- La base no se volvió a correr: su cifra viene del informe de la 97.

## 5. Tests que cambiaron de valor

Ninguno cambió de valor esperado. Los tests viejos de `mutacion.rs`, `memo.rs` y `tests/mutacion.rs` pasan ahora los nombres del reporte como muestra. Dan el mismo resultado.
El helper de test `nombres_del_reporte` pasó a ser una función del módulo. Un reporte ilegible da lista vacía; el fixture `con_reporte` agrega un mutante de respaldo para mantener los motivos de reporte roto.

## 6. Llamadores revisados

- `parsear_outcomes` (firma: suma `pedidos`). codegraph y grep coinciden: `mutacion.rs` (`medir_mutantes` y sus tests) y `tests/mutacion.rs`. Todos se ajustaron.
- `mutantes_muertos` de `mutacion.rs` (firma: suma `pedidos`). Llamadores: `memo.rs:56`, ajustado. El método `MemoSondas::mutantes_muertos` no cambió: lo llaman `calibra.rs:1582`, `calibra/defectos.rs:182` y `mod.rs:7993, 8035`.
- `Lectura::Medible` (variante interna): solo la usan `leer_viables`, `parsear_outcomes` y `mutantes_muertos`.
- `ResultadoMutacion` (campo nuevo): se construye solo en `mutacion.rs`.
- `Evaluada` y `Caso` (campo nuevo): se construyen en `calibra.rs`.
- Resultado de codegraph: el índice se creó con `codegraph init .` y `sync`. Las consultas a `parsear_outcomes` no devolvieron nada útil; manda el `grep -w`.

## 7. Preguntas para el planner

1. **Conectar el aviso.** Hace falta que `mod.rs` lea `fuera_de_la_muestra` y sume el aviso al veredicto. Decisión del organizador: no se tocó `mod.rs`; queda para la tarea que lo posea. `ResultadoMutacion` ya trae los datos.
2. **Los 13 mutantes `StructField` corren siempre.** Decisión del organizador: no se arregla aquí; el cambio los descuenta. Opciones: pasar `--exclude-re "delete field"` a cargo-mutants, o aceptar el costo de tiempo. Decidir también si el listado debe filtrarlos antes de armar la muestra.
3. **Muestra con un `StructField`.** Si la muestra incluye un "delete field", cargo-mutants lo corre junto con los otros 12. El resultado cuenta solo el pedido. Decisión: se acepta.
4. **`aviso_de_mas` quitado.** Decisión del organizador: se quitó la función sin uso. La tarea del punto 1 la repone al conectar el aviso.
