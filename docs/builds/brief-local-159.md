# Brief de la Tarea 159 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (75).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-158` (6ed2525). Trae las Tareas 84 a 158 y la 127b.
- Ningún otro builder corre a la vez. No corras mientras el orquestador use podman.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-159`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el reintento según el lenguaje | `src/oracle/mod.rs` |
| 2, el lock que no cuadra | `src/oracle/descarga.rs` |
| 3, los permisos de GitHub Actions | `src/oracle/configs.rs` |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- **La versión de las sondas.** El punto 3 cambia la conducta de una sonda. Si Root tiene una constante de versión de sondas, subila con la regla que ya existe, y anotalo. Si esa constante vive fuera de tu lista, detenete y anotalo.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

La verificación de las 28 tareas nuevas del dataset encontró tres defectos de Root. El dataset los esquivó, pero el evaluador debe dar la respuesta correcta sin esquivar nada.

**1. El reintento arma ids de pytest también en Rust.** `reintento_de`, en `src/oracle/mod.rs`, arma cada id con `node_id`, que da `dir/archivo.py::nombre`. Eso solo sirve para pytest. Sin `reintento` en `tarea.toml`, además, exige que el comando termine en el directorio de la suite. Un contrato de nextest no termina así. Una tarea Rust con un test inestable da "tarea inválida: el comando no termina en /oraculo/contrato y no tiene reintento".

**2. Un lock que no cuadra sale como error de infraestructura.** `COMANDO_VENDOR`, en `src/oracle/descarga.rs`, corre `cargo vendor --locked`. Con un `Cargo.lock` que no cuadra con su `Cargo.toml`, falla con "cannot update the lock file ... because --locked was passed". Root lo informa con rc 3 y sin veredicto. Es un defecto del producto evaluado: un usuario con `cargo build --locked` falla igual. La lista de diagnósticos de "el producto no instala" ya tiene "no matching package" y "failed to select a version", pero no este caso.

**3. `configs-secretos` marca un permiso de GitHub Actions como secreto.** En un workflow, `id-token: write` dentro de `permissions:` tiene una clave con el segmento "token" y un valor literal. `clave_con_secreto` y `es_literal_secreto`, en `src/oracle/configs.rs`, lo marcan con severidad 3. Cuatro soluciones de referencia de watchexec salen `bloqueado` por ese falso positivo.

## Tarea

La regla que manda: **un evaluador no bloquea una solución correcta, y un defecto del producto da un hallazgo, no un error propio.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El reintento según el lenguaje

1. **`reintento_de` recibe el lenguaje de la tarea.** Uní sus llamadores con codegraph y `grep -w`.
2. **Python:** igual que hoy. `{tests}` se reemplaza por node ids de pytest. Sin `reintento`, el comando debe terminar en el directorio de la suite.
3. **Rust, con `reintento` y `{tests}`:** `{tests}` se reemplaza por una sola expresión de filtro de nextest, entre comillas simples para `sh`.
   - Cada test da `test(=<nombre>)`.
   - Si el id de Root trae también el binario, da `(binary_id(=<binario>) & test(=<nombre>))`.
   - Los tests se unen con ` | `.
4. **El formato del id.** Leé cómo Root arma el id de un test desde el junit de nextest: qué parte sale del `testsuite` y qué parte del `testcase`. Usá esa misma regla para separar el binario del nombre. Citá la función en el informe.
5. **Rust, sin `reintento`:** la corrida extra repite el comando entero, sin cambios. Da resultado para todos los tests, también para los inestables.
6. **Otro lenguaje** da el error de hoy, con el nombre del lenguaje en el mensaje.

**Tests en rojo primero:**
- Unitario: una tarea Rust con `reintento = "... -E {tests}"` y dos inestables da una expresión con `test(=...)` y ` | `, sin `.py::`. Hoy da node ids de pytest.
- Unitario: una tarea Rust sin `reintento`, con un comando que no termina en el directorio, da el comando entero. Hoy da "tarea inválida".
- Unitario: un id con binario da `binary_id(=...) & test(=...)`.
- Guarda: una tarea Python da los mismos node ids que hoy, con y sin `reintento`.

### 2. El lock que no cuadra

1. **"cannot update the lock file" entra a la lista de diagnósticos de "el producto no instala",** junto a "no matching package" y "failed to select a version".
2. **El hallazgo es de Entrega, con severidad 3,** sobre `Cargo.lock`. El texto dice "el producto no instala: Cargo.lock no cuadra con Cargo.toml", con la línea útil del error.
3. **La severidad 3 es la misma** que dan hoy los otros casos de "no instala".
4. **La evaluación sigue como con los otros casos de "no instala":** con veredicto, sin rc 3.
5. **Un error de red o de caché** sigue siendo infraestructura. Solo el texto del lock cambia de camino.

**Tests en rojo primero:**
- Unitario: `diagnosticar` con "error: cannot update the lock file /repo/Cargo.lock because --locked was passed to prevent this" da "el producto no instala", con esa línea. Hoy da infraestructura.
- Unitario: el hallazgo es de Entrega, con severidad 3, sobre `Cargo.lock`.
- Guarda: "no matching package" y "failed to select a version" dan lo mismo que hoy.
- Guarda: un error de red sigue siendo infraestructura.

### 3. Los permisos de GitHub Actions

1. **`es_literal_secreto` da falso** para los valores `read`, `write`, `none`, `read-all` y `write-all`, sin distinguir mayúsculas, con o sin comillas.
2. **La regla vale para YAML y TOML,** con cualquier clave. Ningún secreto real tiene esa forma. Es más simple que reconocer el bloque `permissions:`.
3. **Un valor que parece un secreto sigue contando,** aunque la clave sea `id-token`.

**Tests en rojo primero:**
- Unitario: `id-token: write` en un YAML de `.github/workflows/` no da hallazgo. Hoy da severidad 3.
- Unitario: `contents: read` y `packages: none` no dan hallazgo.
- Guarda: `id-token: "ghp_1234567890abcdef"` sigue dando severidad 3.
- Guarda: los tests de `configs_secretos_*` siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 159.
- La verificación otra vez de las 4 tareas de watchexec bloqueadas, y de watchexec b8d0db4058 con su lock real.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-159.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El id de un test de nextest**: la función que lo arma, y cómo se separan el binario y el nombre.
5. **La versión de las sondas**: si existe, y si cambió.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
