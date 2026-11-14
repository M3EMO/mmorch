# Brief de la Tarea 140 para el builder local

> **SIN USO desde la vuelta (54). No se construye.** La Prueba 0b falló: con `--network host --cgroups=disabled`, crun no puede montar `proc` en Colab. El kernel lo impide (`mount_too_revealing`), porque Docker tapa entradas de `/proc`.

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

**Esta tarea arranca solo si la Prueba 0b del orquestador dio `rc=0`.** La Prueba 0b está en la vuelta (53), sección 3. Si no dio `rc=0`, no arranques: el planner vuelve a decidir.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- La 137, la 138 y la 139 se construyen a la vez o después. Ninguna toca `src/oracle/contenedor.rs`. No toques sus archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-140`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el modo de Colab | `src/oracle/contenedor.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/eficiencia.rs`, `src/oracle/colab.rs`, `src/colab_calibra.rs`, `src/lib.rs`, `src/cli.rs`, `src/corre/`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El usuario decidió mover las mediciones de Root a Colab.** La Prueba 0 del orquestador corrió podman en un runtime de CPU de Colab: Ubuntu 24.04, root con `CAP_SYS_ADMIN` y sin `CAP_NET_ADMIN`.

**Con las opciones de hoy, podman no corre ahí.** `opciones` (`src/oracle/contenedor.rs:141`) arma las opciones de todo contenedor. Dos de ellas fallan en Colab:
- **Los cgroups.** `--pids-limit` y `--memory` piden escribir en `/sys/fs/cgroup`, que en Colab es de solo lectura. `crun` falla.
- **La red aislada.** `--network none` pide levantar la interfaz de loopback, y eso pide `CAP_NET_ADMIN`. Con `--cgroups=disabled` y `--network none`, `crun` falla con `ioctl(SIOCSIFFLAGS): Operation not permitted`.

**La Prueba 0b prueba lo que queda:** `--network host` y `--cgroups=disabled`, con el resto de las opciones de Root. Esta tarea supone que dio `rc=0`.

**Lo que se pierde en este modo, y el riesgo que aceptó el planner:**
- **La red:** el código del contenedor puede salir a internet. El modo vale solo para calibrar en Colab, con broot y los defectos que inyecta Root. En el runtime no hay ninguna clave.
- **Los límites de memoria y de procesos:** un caso que se desborde mata el runtime, que es de un solo uso.
- **`memory.peak`:** sin cgroup no hay. Por eso Eficiencia no se calibra en Colab.
- **Una evaluación de código de un builder sigue en la laptop,** con la red apagada y con límites. Este modo nunca se usa ahí.

## Tarea

La regla que manda: **en Colab, podman corre sin red aislada y sin cgroups, solo si se pide en forma explícita, y con el resto del aislamiento intacto.** Hacé un commit. Empieza con su test en rojo.

### 1. El modo de Colab

1. **Una variable de entorno nueva, `ROOT_CONTENEDOR_MODO`.** Con el valor exacto `colab`, `opciones` usa el modo de Colab. Con cualquier otro valor, o sin la variable, usa el modo de hoy.
2. **El nombre de la variable y su valor** van en constantes públicas, para que la Tarea 139 las use. Su comentario cita la Prueba 0 y la Prueba 0b.
3. **En el modo de Colab, `opciones` cambia tres cosas:**
   - pone `--network host` en vez de `--network none`;
   - suma `--cgroups=disabled`;
   - no pone `--pids-limit` ni `--memory`.
4. **Todo lo demás queda igual:** `--cap-drop=all`, `--security-opt=no-new-privileges`, `--pull=never`, el `/tmp` en `tmpfs`, los montajes de solo lectura, el sufijo de SELinux y las variables de entorno.
5. **Una ejecución que pide red** (`e.red` en verdadero) da lo mismo en los dos modos: sin `--network none`.
6. **La lectura de la variable es inyectable** para los tests. Un test no cambia el entorno del proceso, porque los tests corren en paralelo. Seguí la forma de la Tarea 118 en `src/limpieza.rs`.
7. **Un aviso en stderr,** una vez por proceso, dice que el modo de Colab está activo, y qué aislamiento falta. Así no se usa por error en la laptop.

**Tests en rojo primero:**
- Unitario: con el modo de Colab, `argumentos` trae `--network host` y `--cgroups=disabled`, y no trae `--pids-limit`, `--memory` ni `--network none`.
- Unitario: con el modo de Colab, `argumentos` sigue trayendo `--cap-drop=all`, `--security-opt=no-new-privileges`, el `tmpfs` y los montajes de solo lectura.
- Unitario: sin la variable, o con un valor como `Colab` o `1`, `argumentos` da lo de hoy.
- Unitario: `argumentos_separado` sigue la misma regla.
- Guarda: los tests de endurecimiento y montajes de `contenedor.rs` siguen verdes sin la variable.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **Que una calibración de oráculo corra en Colab con `ROOT_CONTENEDOR_MODO=colab`,** con la Tarea 139.
- **Que en la laptop, sin la variable,** la suite con podman siga verde.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-140.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los argumentos de los dos modos**: los argumentos completos de un `podman run` en el modo de hoy y en el de Colab.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
