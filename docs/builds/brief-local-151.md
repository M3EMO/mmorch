# Brief de la Tarea 151 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (62).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-150`. Trae las Tareas 84 a 150 y la 127b.
- La 142 se construye a la vez. Toca `src/proveedores/mod.rs`, `src/main.rs`, `src/colab_calibra.rs`, `src/cli.rs`, `tools/colab/notebook-juez.py` y `tests/claves_fd.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-151`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, un solo camino de subida | `src/oracle/colab.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe. En `src/colab_calibra.rs`, tocá solo esos tests: la 142 trabaja ahí.
- **No toques:** los archivos de producción de la 142, `src/oracle/eficiencia.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.** Los tests usan el `ColabCli` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Hay dos caminos de subida, y uno no verifica.** La Tarea 150 agregó `subir_en_trozos` en `src/oracle/colab.rs`. El guion de unión del runtime compara el SHA-256 del archivo unido. Pero `subir_binario` sube directo un binario de 40 MiB o menos, sin trozos y sin SHA-256. El `cargo` mínimo también sube directo. El informe de la 150 lo anotó en su pregunta 5.

**La regla de la 150 dice que lo que se sube se verifica.** Un camino sin control rompe esa regla en silencio. Dos caminos son también más superficie para el próximo lector.

## Tarea

La regla que manda: **todo archivo sube por un solo camino, y ese camino verifica el SHA-256.** Hacé un commit. Empieza con su test en rojo.

### 1. Un solo camino de subida

1. **Todo `upload` de `colab.rs` pasa por `subir_en_trozos`,** también los archivos chicos. Un archivo chico va en un solo trozo, como ya hace esa función con el paquete.
2. **Cada subida compara el SHA-256** del archivo unido en el runtime con el de la laptop. Si difiere, la subida falla y nombra el archivo.
3. **El permiso de ejecución** de un binario se fija en el runtime después de la unión, como hoy después de la subida directa.
4. **La rama directa de `subir_binario` sale.** Si `subir_binario` queda como una sola llamada, borrala y llamá a `subir_en_trozos`. Uní sus llamadores con codegraph y `grep -w` antes de borrar.
5. **Ninguna llamada a `upload` queda fuera de esa función.** Buscalas con `grep -n '"upload"' src/`. Anotá cada una en el informe.

**Tests en rojo primero:**
- Unitario: con el `ColabCli` falso, un binario de 1 MiB sube como un trozo y pasa por la unión con SHA-256. Hoy sube directo.
- Unitario: el `cargo` mínimo pasa por la unión con SHA-256. Hoy sube directo.
- Unitario: un SHA-256 distinto en la unión hace fallar la subida del binario, con su nombre en el mensaje.
- Guarda: los tests de la 150 sobre el paquete en trozos siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 142 y la 151.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-151.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las llamadas a `upload`**: cada una, antes y después.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
