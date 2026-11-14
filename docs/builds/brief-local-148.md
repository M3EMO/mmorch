# Brief de la Tarea 148 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (60) y (61).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-147`. Trae las Tareas 84 a 147 y la 127b.
- La 149 se construye a la vez. Toca `src/oracle/eficiencia.rs`. No toques ese archivo.
- La 150 se construye después. Toca `src/colab_calibra.rs`, `src/oracle/colab.rs`, `src/lib.rs` y `tools/colab/verificar-versiones.sh`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-148`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la lista de la sonda por archivo | `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs` |
| 2, el texto de `MOTIVO_LASTRE_SIN_LUGAR` | `src/oracle/calibra.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 149 y la 150, `src/oracle/contenedor.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**1. La sonda pierde el inicio de su lista.** En la calibración v5, los 6 defectos de Eficiencia quedaron no aplicables. La sonda corrió bien en el contenedor. El journal muestra `ROOT_SONDA_INICIO`, 203 funciones y `ROOT_SONDA_FIN`. Pero `resultado_de`, en `src/oracle/contenedor.rs`, guarda solo la cola de 4000 caracteres del stdout (`COLA`). El bloque mide de 4514 a 6205 caracteres. La cola pierde `ROOT_SONDA_INICIO`. Sin inicio, `salida_valida` da falso, y `medir_lastre` da "sin salida; el comando no llegó a sus marcas". El test de punta a punta de la 143 no lo vio: su ejecutor falso devuelve el stdout entero, y su repo tiene cuatro funciones.

**2. Un motivo habla de una regla que ya no existe.** `MOTIVO_LASTRE_SIN_LUGAR` dice "función vieja". Desde la 143, el lugar del lastre sale de la sonda. La 147 borró la regla de la función vieja. El informe de la 147 lo anotó en su pregunta 3.

## Tarea

La regla que manda: **una medida no depende del tamaño de un stdout recortado, y un motivo nombra la regla que corre.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La lista de la sonda por archivo

1. **`cmd_sonda` escribe la lista en un archivo.** Después de los tests, corre `ls /tmp/root-sonda > /salida/root-sonda.lista`. `/salida` es la carpeta que `medir_lastre` pasa a `ejecutar_en`.
2. **`medir_lastre` lee ese archivo** en `ruta_salida`, después de la corrida. Sin archivo, da el error de hoy: "sonda de lastre: sin salida; el comando no llegó a sus marcas".
3. **`leer_salida` lee el texto del archivo,** una función por línea. Mantiene el filtro por el mapa de funciones marcadas.
4. **Las marcas `ROOT_SONDA_INICIO` y `ROOT_SONDA_FIN` salen** del comando, junto con `tramo` y `salida_valida`, si quedan sin uso. Uní sus llamadores con codegraph y `grep -w` antes de borrar.
5. **Revisá los otros lectores del stdout.** Buscá cada código que busque una marca de inicio en un `stdout` de `ejecutar` o `ejecutar_en`. Anotá cada uno en el informe, con su tamaño posible. No los cambies si están fuera de tu tabla.

**Tests en rojo primero:**
- Unitario: un ejecutor falso que deja solo los últimos 4000 caracteres del stdout, como `resultado_de`. La sonda marca 300 funciones, y todas corren. `medir_lastre` debe dar las 300. Hoy da el error de las marcas.
- Unitario: el ejecutor falso no escribe el archivo. `medir_lastre` da "el comando no llegó a sus marcas".
- Guarda: el test de punta a punta de la 143 sigue verde. Su ejecutor falso pasa a escribir el archivo.

### 2. El texto de `MOTIVO_LASTRE_SIN_LUGAR`

1. **El texto nombra la sonda.** Dice que la sonda no encontró ninguna función que corra en los tests comunes. No habla de funciones viejas.
2. **Su test cambia junto con el texto,** en el mismo commit.
3. **Buscá el literal viejo** en todo el repo con `grep`, también en `docs/`. Anotá cada lugar en el informe. Un documento fuera de tu tabla se anota, no se toca.

**Test en rojo primero:**
- Unitario: el motivo no contiene "vieja". Hoy lo contiene.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 148, la 149 y la 150.
- La calibración de Eficiencia en Colab. Ahí se ve si la sonda da su lista entera.

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

Al terminar, escribí `docs/builds/informe-local-148.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lo que se borró**: cada función o constante, con sus llamadores antes de borrar.
5. **Los lectores del stdout**: cada uno, con su marca y su tamaño posible.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
