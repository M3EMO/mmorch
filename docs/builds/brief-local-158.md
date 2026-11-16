# Brief de la Tarea 158 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (72).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-157` (e839d74). Trae las Tareas 84 a 157 y la 127b.
- **Arrancá cuando termine la suite con podman de la cadena 157.** No corras mientras el orquestador use podman.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-158`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la guarda de secretos en `script_trabajo` | `src/colab_calibra.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/lib.rs`, `src/cli.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La regla de las claves:** una clave entra a Colab solo por Colab Secrets. Nunca va en un archivo, en un argumento ni en un comando.

**`script_trabajo` escribe el entorno del trabajo dentro de `trabajo-N.py`,** y ese guion sube al runtime. Hoy el entorno sale de `comando_de_trabajo`, y lleva solo tres variables: `ROOT_CONTENEDOR_MODO`, `ROOT_BWRAP_ENTORNO` y `ROOT_BWRAP_IMAGEN_ID`. Ninguna es secreta.

**Nada impide que mañana entre una clave.** El informe de la 157 lo anotó en su pregunta 6. Un cambio en `comando_de_trabajo` podría sumar una variable con valor secreto, y el guion la llevaría a Colab sin aviso. La guarda cierra ese camino en el último punto antes del archivo.

## Tarea

La regla que manda: **ningún guion que sube a Colab lleva el valor de una variable con nombre secreto.** Hacé un commit. Empieza con su test en rojo.

### 1. La guarda de secretos en `script_trabajo`

1. **`script_trabajo` revisa cada variable del entorno** con `es_nombre_secreto`, antes de escribir el guion.
2. **Una variable con nombre secreto y con valor** hace fallar el armado con `ErrorOraculo::Uso`. El mensaje nombra la variable y nunca muestra el valor.
3. **La falla ocurre antes de escribir nada.** No queda un guion a medias en el disco.
4. **Si `script_trabajo` devuelve hoy un `String` y no un `Result`,** cambiá su firma a `Result`. Uní sus llamadores con codegraph y `grep -w`, y propagá el error.
5. **`ROOT_TRABAJO_CMD` de la 157 no cambia.** Sigue mostrando el nombre con valor `null`.

**Tests en rojo primero:**
- Unitario: un entorno con `GITHUB_TOKEN=valor-falso` hace fallar `script_trabajo`, y el mensaje no contiene `valor-falso`. Hoy el guion se escribe con el valor.
- Unitario: con la falla, no queda ningún `trabajo-N.py` en la carpeta temporal.
- Guarda: el entorno de `comando_de_trabajo` pasa la guarda sin cambios.
- Guarda: los tests de `ROOT_TRABAJO_CMD` de la 157 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 158.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-158.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los llamadores de `script_trabajo`**: cada uno, y cómo propaga el error.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
