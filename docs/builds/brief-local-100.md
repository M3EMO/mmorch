# Brief de la Tarea 100 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Base: punta de la última tarea mezclable. El orquestador completa el commit: `________`. Hoy es 7af9a54, la de la Tarea 97.
- Las Tareas 84 a 97 todavía no están mezcladas. La 98 y la 99 se construyen a la vez, en otras ramas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-100`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, solo los mutantes pedidos | `src/oracle/mutacion.rs`; `src/oracle/calibra.rs` solo para guardar el filtro en el caso; `docs/interfaces.md` |
| 2, diagnóstico de nombres | ningún archivo del repo: solo el informe |

- **No toques los archivos de la 98 y la 99:** `src/oracle/calibra/inyectar.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs` y `src/tipos.rs`.
- Si tocás `calibra.rs`, tocalo lo mínimo y explicalo en el informe.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- **Podman, solo para el punto 2.** Si el orquestador no corre otra suite, podés correr cargo-mutants con podman sobre una copia de broot. Preguntale antes de empezar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El reporte trae mutantes que Root no pidió.** El orquestador calibró Pruebas en t1 de S2 con el binario de la 97. El detalle está en `/home/mpera/.cache/root-wt/orquestador/calibracion-pruebas-97.md`.
- La muestra del limpio trae 27 mutantes. 14 caen en `src/pattern/composite_pattern.rs`, el único archivo del cambio.
- Los otros 13 caen fuera del cambio, en archivos iguales en `bootstrap/` y en el repo bueno. Los 13 son del tipo "delete field X from struct Y expression".
- El caso no viene de una copia reusada: `limpio_reusado` es falso.

**Lo que hace hoy la sonda** (`src/oracle/mutacion.rs`):
- `medir_mutantes` filtra el listado con `dentro_del_cambio` (línea 614), arma la muestra, y corre cargo-mutants con un `-F` exacto (`filtro_de`).
- `parsear_outcomes` (línea 317) cuenta todos los mutantes del `outcomes.json`, sin compararlos con la muestra pedida.
- `mutantes_muertos`, que alimenta a `assert_borrado`, lee el mismo reporte.

**La causa no está encontrada.** Puede ser que cargo-mutants nombre distinto ese tipo de mutante en `--list` y en la corrida, o que aplique `-F` a otro texto. No hay que adivinarla. Hay que dejar de confiar en que el reporte trae solo lo pedido.

## Tarea

La regla que manda: **el puntaje se calcula sobre los mutantes que Root eligió, y nada más.** Hacé un commit por el punto 1. El punto 2 va solo en el informe.

### 1. Solo los mutantes pedidos

1. `parsear_outcomes` y `mutantes_muertos` reciben la muestra pedida. Solo cuentan los mutantes cuyo nombre está en ella.
2. **Mutantes de más.** Un mutante del reporte que no estaba en la muestra no cuenta. Queda en `fuera_de_la_muestra`, una lista nueva de `ResultadoMutacion` con `#[serde(default)]`. El veredicto suma el aviso "cargo-mutants corrió N mutantes que no estaban en la muestra".
3. **Mutantes de menos.** Un mutante pedido que no aparece en el reporte deja Pruebas sin veredicto, con el motivo "el reporte no trae el mutante pedido <nombre>".
4. **El filtro.** Cada caso de calibración de Pruebas guarda el `-F` que se usó.
5. Los llamadores de test que hoy pasan un reporte sin muestra siguen funcionando. Si hace falta, pasan la lista de nombres del reporte.
6. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un reporte con 3 mutantes pedidos y 2 de más cuenta solo los 3, y lista los 2 en `fuera_de_la_muestra`.
- Unitario: un reporte al que le falta un mutante pedido deja Pruebas sin veredicto, con su nombre.
- Unitario: `mutantes_muertos` no devuelve un mutante que no estaba en la muestra.
- Guarda: un reporte con exactamente la muestra da el mismo puntaje que hoy.

### 2. Diagnóstico de los nombres

1. Sobre una copia de `~/.cache/root-wt/mineria/repos/Canop_broot` en el commit de t1 (`1f77962e6ff3`), dentro de tu árbol de trabajo, corré `cargo mutants --list --json` con la imagen de los tests.
2. Elegí un mutante "delete field ... from struct ... expression" del listado. Corré cargo-mutants con un `-F` de solo ese nombre, armado con `filtro_de`.
3. Compará el nombre del listado con los nombres del `outcomes.json`. Anotá también cuántos mutantes corrió.
4. Repetí con un mutante de otro tipo, por ejemplo "replace ... with ...".
5. Informá lo que encontraste. Si encontrás la causa, no la arregles en esta tarea: escribila en "Preguntas para el planner".
6. Solo leé ese repo. No lo modifiques.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-100.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con defecto, tests vistos en rojo y commit.
3. **Diagnóstico de los nombres**: los nombres del listado y del reporte, cuántos mutantes corrieron con cada `-F`, y la causa si la encontraste.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
