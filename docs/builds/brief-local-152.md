# Brief de la Tarea 152 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (63).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-150`. Trae las Tareas 84 a 150 y la 127b.
- La 142 y la 151 se construyen a la vez. La 142 toca `src/proveedores/mod.rs`, `src/main.rs`, `src/colab_calibra.rs`, `src/cli.rs`, `tools/colab/notebook-juez.py` y `tests/claves_fd.rs`. La 151 toca `src/oracle/colab.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-152`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el `tmpfs` en `/oraculo` y la revisión de destinos | `src/oracle/contenedor.rs` |
| 2, una sola fuente para `CARPETAS_RUNTIME` | `src/oracle/contenedor.rs`, `tests/colab_preparar.rs` |

- Los dos puntos comparten `contenedor.rs`. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 142 y la 151, `tools/colab/preparar-runtime.sh`, `tools/colab/verificar-versiones.sh`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**En Colab, el contrato no corre.** La prueba de la 141 con la cadena 150 llegó hasta la importación. La importación frenó: "la base de Colab difiere de la base de la laptop: colab=[0.111111164] laptop=[1.0]". El stderr del contenedor dice: "bwrap: Can't mkdir /oraculo/contrato: Read-only file system".

**bwrap no crea un punto de montaje dentro de una raíz de solo lectura.** `argumentos_bwrap` monta `/` con `--ro-bind`. Cada destino debe existir ya en el runtime. `tools/colab/preparar-runtime.sh` crea las seis carpetas de `CARPETAS_RUNTIME`: `/repo`, `/salida`, `/oraculo`, `/cache`, `/rustsec` y `/trabajo`.

**`oraculo_destino` lleva el oráculo a una ruta que no está en la lista.** Tiene tres valores: `/oraculo/contrato` y `/oraculo/oculto`, de `ejecucion_de_suite` en `src/oracle/mod.rs`, y `/oraculo/escenarios`, de `ejecucion_de_evaluacion` en `src/oracle/entrega.rs`. Toda calibración evalúa primero el repo bueno, y esa evaluación corre el contrato. Por eso fallan las cinco dimensiones de Colab.

**Una lista más larga no alcanza.** Un destino nuevo, armado en otro archivo, volvería a romper Colab en silencio. bwrap sí crea carpetas dentro de un `tmpfs` que él mismo armó. Con un `tmpfs` en `/oraculo`, todo destino debajo funciona.

**La lista tiene hoy tres copias a mano.** Están en los dos guiones y en `tests/colab_preparar.rs`. `contenedor.rs` solo la nombra en un comentario.

## Tarea

La regla que manda: **bwrap recibe solo destinos que puede montar, y el código lo revisa antes de lanzarlo.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El `tmpfs` en `/oraculo` y la revisión de destinos

1. **Antes del montaje del oráculo, `argumentos_bwrap` agrega `--tmpfs /oraculo`.** Va siempre que haya oráculo, también con el destino `/oraculo`. El `tmpfs` va antes del `--ro-bind` del oráculo.
2. **`argumentos_bwrap` revisa cada destino de montaje.** Un destino vale si está en `CARPETAS_RUNTIME`, o si está bajo `/oraculo/`. Si no, da `ErrorOraculo::Infra` con un texto que nombra el destino. No lanza bwrap.
3. **El modo podman no cambia.** Sus argumentos quedan letra por letra.
4. **Un comentario explica el `tmpfs`:** bwrap no crea un punto de montaje en una raíz de solo lectura, pero sí dentro de un `tmpfs` suyo. Citá el mensaje de Colab.

**Tests en rojo primero:**
- Unitario: para `oraculo_destino` en `None`, `/oraculo/contrato`, `/oraculo/oculto` y `/oraculo/escenarios`, cada destino de los argumentos está en `CARPETAS_RUNTIME`, o bajo un `--tmpfs` que aparece antes en los mismos argumentos. Hoy falla con `/oraculo/contrato`.
- Unitario: los valores `/oraculo/contrato` y `/oraculo/oculto` del test salen de `DIR_CONTRATO` y `DIR_OCULTO`. Si no se alcanzan desde `contenedor.rs`, hacelos `pub(crate)` en el test, sin cambiar su valor, o anotalo.
- Unitario: un destino `/otra` da error con su nombre, y no arma argumentos.
- Guarda: los argumentos de podman no cambian.
- Guarda: los tests de bwrap de la 139 siguen verdes, salvo el orden nuevo del `tmpfs`. Anotá cada uno que cambie.

### 2. Una sola fuente para `CARPETAS_RUNTIME`

1. **`contenedor.rs` define `pub const CARPETAS_RUNTIME`,** con las seis carpetas de hoy. `argumentos_bwrap` la usa en la revisión del punto 1.
2. **`tests/colab_preparar.rs` borra su copia** y usa la constante del crate.
3. **El test de los guiones compara** las dos copias de los guiones con la constante del crate. Ya existe un test que lee la línea `CARPETAS_RUNTIME=(` de un guion. Que lea los dos.
4. **Los comentarios de los guiones no cambian.** No toques los guiones. Si un comentario queda falso, anotalo.

**Tests en rojo primero:**
- Integración: el test de los guiones lee la constante del crate. Hoy lee su propia copia.
- Guarda: si un guion tuviera otra lista, el test falla y nombra el guion. Probalo con un texto falso en un temporal del test.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 151 y la 152.
- La prueba de la 141 en Colab otra vez. La base de Colab debe dar 1,0, como la de la laptop.

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

Al terminar, escribí `docs/builds/informe-local-152.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los destinos de montaje**: cada lugar que llena `oraculo_destino`, con su valor, y cada llamada a `montaje_bwrap`.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
