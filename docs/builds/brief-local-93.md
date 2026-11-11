# Brief de la Tarea 93 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-92` (1ab1ac5). Las Tareas 84 a 92 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-93`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, evaluación sin root | `src/oracle/contenedor.rs`, `src/oracle/entrega.rs`, `tests/entrega.rs` |
| 2, cupo por rol y por vuelta | `src/corre/corrida.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite completa con podman corre al mezclar, y la corre el orquestador.
- **Podman, solo para la entrega.** Si el orquestador no corre otra suite, el organizador puede correr con podman `tests/entrega.rs` y los tests de `entrega` de la biblioteca. Preguntale antes de empezar.
- **Imagen de Python.** `docker.io/library/python:3.12-slim` está en la laptop. Revisalo con `podman image exists docker.io/library/python:3.12-slim`. No bajes imágenes nuevas.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El contenedor de evaluación corre como root.** `imagenes/oraculo-rust/Containerfile` no declara `USER`. `argumentos_separado` (`src/oracle/contenedor.rs:124`) no pasa `--user`. Un test lo fija: `argumentos_separado_deja_el_contenedor_vivo_y_sin_rm` (`contenedor.rs:1313`). Lo introdujo el commit 10a58b7, del punto 2 de la Tarea 91.
- El contenedor de evaluación vive durante todos los pasos. `instalar` es código del producto.
- Como root, `instalar` puede reemplazar `/bin/sh` o `/usr/bin/timeout`.
- Después, el envoltorio de Root corre ese programa falso en cada escenario, y puede salir con 0.
- Las rutas absolutas de la Tarea 92 no protegen contra eso.
- El host no corre riesgo: con podman sin root, el root del contenedor es el usuario del host. El riesgo es un veredicto falso.
- Las suites no tienen el problema: cada una corre en un contenedor nuevo, con `--rm`.

**El `sh` externo.** El informe de la Tarea 92 dejó el `sh` de `podman exec <c> sh -c ...` sin ruta absoluta. La sesión ya detecta esa ruta.

**Python sin probar con podman.** El informe de la 92 dijo que no había imagen local de Python. Sí la hay.

**MEDIUM 8, cupo.** `r00t corre` rutea todos los roles una vez, al principio, con `TOKENS_RUTEO` (`src/corre/corrida.rs:172` a `178`). Si el cupo se agota durante la corrida, los roles siguientes se lanzan igual, `claude -p` choca contra el límite, y la corrida se pierde a mitad de camino.

## Tarea

La regla que manda: **el código del producto nunca puede cambiar los programas que usa Root, y una corrida no se lanza sin cupo.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Evaluación sin root

1. **Primero, el motivo.** Mirá el commit 10a58b7 y su informe (`docs/builds/informe-local-91.md`). Buscá por qué el test fija "sin `--user`". Si había un motivo, como permisos de montaje, informalo antes de cambiarlo.
2. **Sin root.** El contenedor de evaluación arranca con `--userns=keep-id`. Su usuario es el del host, sin privilegios adentro.
3. **Montajes.** `/salida`, la carpeta de trabajo y la caché tienen que seguir escribibles para ese usuario. `/tmp` es un tmpfs: comprobá que ese usuario puede escribir ahí.
4. **Solo la evaluación.** Las suites y los contenedores de `ejecutar` no cambian en esta tarea.
5. **`sh` externo.** `podman exec` usa la ruta absoluta de `sh` que detecta la sesión.
6. Cambiá el test de la línea 1313 a la regla nueva, y explicalo en el informe.

**Tests en rojo primero:**
- Unitario: los argumentos de arranque del contenedor de evaluación traen `--userns=keep-id`.
- Unitario: los argumentos de un paso usan la ruta absoluta de `sh` en el `podman exec`.
- Con podman: un `instalar` que intenta `cp /bin/true /bin/sh` falla por permisos. Un escenario que debe fallar sigue fallando.
- Con podman, en `docker.io/library/python:3.12-slim`: con el preludio de Python, un `instalar` que deja un `sh` falso en `/tmp/venv-eval/bin` no hace pasar un escenario que debe fallar. `python` sale del venv.
- Con podman: el preludio de Rust sigue funcionando sin root, con la imagen de los tests de hoy.
- Guarda: los tests de entrega con podman siguen verdes.

### 2. Cupo por rol y por vuelta

1. Antes de cada lanzamiento, por rol y por vuelta, Root rutea otra vez con `rutear_en` y `exigir_cupo`.
2. Si el rol ya no va por cupo, la corrida para con el motivo "cupo agotado antes del rol X, vuelta N".
3. La fila de `runs.jsonl` guarda lo que se hizo hasta ahí, como un corte de hoy.
4. La copia de la credencial se borra igual, por `GuardiaCredencial`.
5. El ruteo inicial de todos los roles sigue igual.

**Tests en rojo primero:**
- Unitario: con un ruteo falso que va por cupo para el primer rol y por API para el segundo, el segundo rol no se lanza, y la fila trae el motivo.
- Unitario: con un ruteo que deja de ir por cupo antes de la segunda vuelta de un rol, esa vuelta no se lanza.
- Guarda: con cupo durante toda la corrida, los roles se lanzan como hoy.

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

Al terminar, escribí `docs/builds/informe-local-93.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`, y los tests de entrega con podman si corrieron.
2. **El motivo de "sin `--user`"**: lo que encontraste en 10a58b7 y en su informe.
3. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
