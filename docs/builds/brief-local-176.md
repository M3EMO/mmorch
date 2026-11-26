# Brief de la Tarea 176 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (103).

**Este brief vale con P133 c.** Con otra respuesta, el planner lo cambia antes de lanzarlo. Además, la sonda 10 de la vuelta 103 debe salir bien antes de lanzarlo.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-175`.** Trae 5a4f295 y las Tareas 171 a 175. La arma el orquestador después de la suite de la 175. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-176`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- **El builder arregla los defectos de Root que encuentra en esta tarea, por P121 a y P122 a.** Si un arreglo pide un archivo fuera de la tabla, se detiene y lo anota.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Por qué

**En Colab, nadie puede montar un procfs para un espacio de pids nuevo.** Las sondas 6 a 9 lo midieron:
- `bwrap --unshare-pid --proc /proc` falla con `Can't mount proc on /newroot/proc: Permission denied`, como cualquier usuario y en cualquier espacio de red;
- root, con `CAP_SYS_ADMIN`, tampoco puede: `mount` devuelve `EACCES`;
- sí anda montar un procfs del espacio de pids del runtime, y `--ro-bind` de un procfs ya montado.

**Hoy `src/corre/bwrap.rs` pasa siempre `--unshare-pid` y `--proc /proc`.** Por eso el builder no arranca en Colab.

**El oráculo ya resolvió esto.** `src/oracle/contenedor.rs` corre con `--unshare-pid` y `--ro-bind /proc /proc`, y así se calibró en Colab. Pero el builder es código no confiable. Con el `/proc` entero del runtime, vería todos los procesos del runtime, y la línea de comandos de cada uno.

**Por P133 c, el builder ve un procfs del runtime montado con `hidepid=invisible`.** Con esa opción, un usuario solo ve sus propios procesos. Así, el builder ve lo mismo que en podman: sus procesos, y nada más.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el `/proc` del builder en bwrap | `src/corre/bwrap.rs` |
| 2, el procfs con `hidepid` en el runtime | `tools/colab/preparar-corre.sh`, `tools/colab/corre-runtime.py`, y los tests en `src/colab_corre.rs` |

- Los dos puntos no comparten archivos. Van en serie: 1 y 2.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/corre/contenedor.rs`, `src/corre/ejecutor.rs`, el resto de `src/`, `tools/colab/red-builder.sh`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **El builder de podman de la laptop no cambia.** Esta tarea solo toca el ejecutor de bwrap.
- **Ninguna huella cambia.** **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **Ningún test monta un procfs, ni corre `mount` real.** Los tests usan textos de `mountinfo` falsos y programas falsos.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **en bwrap, el builder tiene su propio espacio de pids, y su `/proc` solo muestra sus procesos.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El `/proc` del builder en bwrap

1. **Una variable nueva, `ROOT_PROC_BUILDER`,** nombra un procfs ya montado.
2. **Con la variable,** los argumentos de bwrap cambian `--proc /proc` por `--ro-bind <ruta> /proc`. **Sin la variable,** quedan como hoy, con `--proc /proc`.
3. **`--unshare-pid` queda siempre.** Así, cuando `claude` termina, todos los procesos del builder mueren con su espacio de pids.
4. **El chequeo previo suma, con la variable:**
   - la ruta es el punto de montaje de un procfs, según `/proc/self/mountinfo`;
   - sus opciones traen `hidepid=invisible` o `hidepid=2`.
   Si falla, es un error de infraestructura que nombra la ruta y lo que falta. **Así, un procfs sin `hidepid` nunca llega al builder.**
5. **La lectura de `mountinfo` es una función pura,** que recibe el texto y la ruta. Así se prueba sin montar nada.
6. **El veredicto de la corrida suma un aviso:** "builder con /proc del runtime y hidepid".

**Tests en rojo primero:**
- Unitario: con `ROOT_PROC_BUILDER`, los argumentos traen `--ro-bind <ruta> /proc`, y no traen `--proc`. Hoy la variable no existe.
- Unitario: sin la variable, los argumentos traen `--proc /proc`, como hoy.
- Unitario: con o sin la variable, los argumentos traen `--unshare-pid`.
- Unitario: un `mountinfo` falso con un procfs con `hidepid=invisible` pasa el chequeo.
- Unitario: un procfs sin `hidepid` frena el lanzamiento, con un error que lo dice.
- Unitario: una ruta que no es un procfs, por ejemplo un tmpfs, frena el lanzamiento.
- Unitario: con la variable, el veredicto trae el aviso.
- Guarda: los montajes de bwrap, salvo el de `/proc`, son los mismos que en la base.
- Guarda: los tests de `EjecutorContenedorBuilder` siguen verdes.

### 2. El procfs con `hidepid` en el runtime

1. **`preparar-corre.sh` suma un paso `proc-builder`,** después de `red-builder`:
   - crea `/run/root-proc`, de root, con modo 0755;
   - si ahí no hay un procfs montado, corre `mount -t proc -o hidepid=invisible,nosuid,nodev,noexec proc /run/root-proc`;
   - revisa en `/proc/self/mountinfo` que el montaje existe con `hidepid`. Si no, sale con 1 y lo dice.
2. **Es idempotente:** con el montaje ya hecho, no monta otra vez.
3. **`corre-runtime.py` fija `ROOT_PROC_BUILDER=/run/root-proc`** en el entorno del `r00t` hijo.

**Tests en rojo primero, con `mount` falso y `mountinfo` falso:**
- Unitario de guion: el paso `proc-builder` llama a `mount -t proc -o hidepid=invisible,nosuid,nodev,noexec proc /run/root-proc`. Hoy el paso no existe.
- Unitario de guion: con el montaje ya presente, no llama a `mount`.
- Unitario de guion: si después del montaje `mountinfo` no muestra `hidepid`, el guion frena.
- Unitario de Python: el entorno del `r00t` hijo trae `ROOT_PROC_BUILDER=/run/root-proc`.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **Antes de lanzar este builder:** la sonda 10 de la vuelta 103, y la respuesta del usuario a P133.
2. **La suite con podman** de `laptop/cadena-176`.
3. **`git diff laptop/cadena-175 laptop/tarea-176 --stat`** toca solo los archivos de la tabla y el informe.
4. **La prueba de aislamiento en Colab,** con el release de `laptop/cadena-176`, y los ataques A15 y A16 de la vuelta 103.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-176.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El `/proc` del builder**: lo que ve en la laptop, en Colab con la variable, y en Colab sin ella.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
