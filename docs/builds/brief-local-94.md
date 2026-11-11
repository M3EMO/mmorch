# Brief de la Tarea 94 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-93` (89ee1f6). Las Tareas 84 a 93 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-94`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, carpeta temporal de `calibra` | `src/oracle/calibra.rs` |
| 2, modelo por lanzamiento y causa del `Gate` | `src/corre/corrida.rs`, `docs/interfaces.md` |
| 3, clippy | los archivos que cambió la Tarea 93, solo para los avisos de clippy |

- Los puntos 1 y 2 no comparten archivos. Corren en paralelo. El punto 3 va al final.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**`calibra` usa `/tmp`.** `calibrar_medida` crea su carpeta temporal con `tempfile::tempdir()` (`src/oracle/calibra.rs:947`). Ahí guarda el `target` de la base (Tarea 59). En Fedora, `/tmp` vive en memoria. El `target` de un repo como broot pesa varios gigas. Una calibración de D4 sobre broot puede llenar la RAM o fallar sin espacio. Colab, la descarga y la evaluación ya usan `ROOT_HOME/tmp`, con la función `temporal` de `src/oracle/mod.rs:2004` y la constante `CARPETA_TEMPORALES`.

**La fila guarda el modelo equivocado.** La Tarea 93 rutea otra vez antes de cada lanzamiento, y el lanzamiento usa el modelo del ruteo nuevo. La fila de `runs.jsonl` sigue guardando los modelos del ruteo inicial. Root es primero un banco de medición: la fila tiene que decir qué modelo corrió de verdad.

**El `Gate` de cupo no dice su causa.** Si la vía pasa a API sin `SinPresupuesto`, el `Gate` de "cupo agotado" no dice por qué.

**Clippy no corrió en la Tarea 93.**

## Tarea

La regla que manda: **lo pesado vive en disco, y la fila dice lo que de verdad corrió.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 3.

### 1. Carpeta temporal de `calibra`

1. La carpeta temporal de `calibrar_medida` se crea bajo `ROOT_HOME/tmp`, con `temporal` y `CARPETA_TEMPORALES`.
2. Si `calibrar_medida` no recibe `home`, pasalo desde su llamador. Uní antes los llamadores de codegraph y de grep.
3. La carpeta se borra al terminar, como hoy, también si la calibración falla.
4. Buscá otros `tempfile::tempdir()` fuera de tests en `src/oracle/calibra.rs` y en `src/oracle/calibra/`. Si hay, van al mismo lugar. Anotalos en el informe.

**Tests en rojo primero:**
- Unitario: una calibración con un `home` de prueba crea su carpeta temporal dentro de `home/tmp`, y no en el `/tmp` del sistema.
- Unitario: después de la calibración, la carpeta ya no existe.
- Guarda: los tests de calibración siguen verdes.

### 2. Modelo por lanzamiento y causa del `Gate`

1. La fila de `runs.jsonl` guarda, por rol, el modelo del último lanzamiento que corrió. Si cambió entre vueltas, guarda también la lista de modelos por vuelta.
2. Los campos nuevos llevan `#[serde(default)]`. Una fila vieja se lee igual.
3. El `Gate` de "cupo agotado" nombra la vía elegida y el motivo del ruteo, sea cual sea.
4. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: con un ruteo que cambia de modelo entre la primera y la segunda vuelta de un rol, la fila guarda los dos modelos y el último.
- Unitario: el `Gate` de un rol que pasó a API sin `SinPresupuesto` nombra la vía y su motivo.
- Guarda: una fila vieja sin los campos nuevos se lee igual.

### 3. Clippy

1. Corré `cargo clippy --all-targets` sobre la punta.
2. Arreglá los avisos que vienen de la Tarea 93, sin cambiar comportamiento.
3. Un aviso que pide cambiar comportamiento no se arregla. Se anota en el informe.

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

Al terminar, escribí `docs/builds/informe-local-94.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`, y el resultado de clippy.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Otros `tempdir()` encontrados**, y dónde quedaron.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
