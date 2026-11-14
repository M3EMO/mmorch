# Brief de la Tarea 139 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`. Reemplaza la versión de la vuelta (53).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- **Arrancá cuando la serie 138 termine de medir.** El orquestador lo dice.
- La 140 y la 141 se construyen a la vez. La 140 toca `tools/colab/` y `tests/colab_preparar.rs`. La 141 toca `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs` y `src/oracle/colab.rs`. No toques esos archivos.
- La 137 y la 138 tocan `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/sonda_lastre.rs`, los tests de `src/oracle/calibra/defectos.rs` y `src/oracle/eficiencia.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-139`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el armado de los argumentos de `bwrap` | `src/oracle/contenedor.rs` |
| 2, `ejecutar` con `bwrap` | `src/oracle/contenedor.rs` |
| 3, los otros lugares que llaman a podman | `src/oracle/descarga.rs` (solo `correr_podman_acotado` y quien elige ese camino), `src/oracle/calibra/target.rs` (solo `digest_de_imagen`), `src/oracle/contenedor.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 137, la 138, la 140 y la 141, `src/limpieza.rs`, `src/oracle/entrega.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La decisión del usuario, literal:** P110 "b) Bubblewrap". Las cuatro dimensiones de oráculo (Seguridad, Funciona, Completitud y Pruebas) se calibran en Colab, con bubblewrap.

**En Colab no corre ningún contenedor anidado.** El runtime es un contenedor de Docker sin privilegios. El kernel no deja montar un `/proc` nuevo (`mount_too_revealing`), porque Docker tapa entradas de `/proc`. podman y `unshare --mount-proc` fallan.

**bubblewrap sí anda, si no monta un `/proc` nuevo.** La Prueba 0d del orquestador corrió bubblewrap 0.9.0 en Colab:
```bash
COMUN="--ro-bind / / --dev-bind /dev /dev --ro-bind /proc /proc --tmpfs /tmp --ro-bind /content/p/ro /repo --bind /content/p/sal /salida"
bwrap --unshare-user --uid 0 --gid 0 --unshare-net --unshare-pid $COMUN -- sh -c '...'
```
- El repo quedó de solo lectura, y la salida escribible.
- La red quedó cerrada: `Network is unreachable`.
- El usuario y los procesos fueron propios: `pid=2`.

**Lo que hoy hace `contenedor::ejecutar`** (`src/oracle/contenedor.rs:222`): arma los argumentos de `podman run` con `argumentos` y `opciones`, lanza `podman`, y al vencer el plazo borra el contenedor por nombre. Las tareas de S2 corren comandos como `cd /repo && ... cargo nextest run ... --tool-config-file root:/etc/root-oraculo/nextest.toml`. Esperan `/repo`, `/oraculo`, `/cache`, `/rustsec` y `/salida` en esas rutas.

**Lo que se pierde con `bwrap`:** los límites de memoria y de procesos (`--memory`, `--pids-limit`) y `memory.peak`. Por eso este modo sirve solo para las dimensiones de oráculo, en un runtime de un solo uso. Eficiencia no se calibra así.

## Tarea

La regla que manda: **con `ROOT_CONTENEDOR_MODO=bwrap`, cada ejecución corre en bubblewrap con los mismos montajes, la red cerrada y el entorno de la imagen. Sin la variable, todo sigue con podman.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El armado de los argumentos de `bwrap`

1. **Una variable nueva, `ROOT_CONTENEDOR_MODO`.** Con el valor exacto `bwrap`, el backend es bubblewrap. Con cualquier otro valor, o sin la variable, es podman.
2. **El nombre y el valor** van en constantes públicas, para que la Tarea 141 las use.
3. **La lectura de la variable es inyectable** para los tests, como en la Tarea 118 (`src/limpieza.rs`). Un test no cambia el entorno del proceso.
4. **Una función pura, `argumentos_bwrap(e: &Ejecucion, entorno: &[(String, String)])`,** da los argumentos, en este orden:
   - `--die-with-parent`, `--unshare-user`, `--uid 0`, `--gid 0`, `--unshare-pid`;
   - `--unshare-net`, salvo que `e.red` sea verdadero;
   - `--ro-bind / /`, `--dev-bind /dev /dev`, `--ro-bind /proc /proc`, `--tmpfs /tmp`;
   - los montajes de `e`, en las mismas rutas que podman: `--ro-bind` para `/repo`, `/trabajo`, `/oraculo` (o `e.oraculo_destino`), `/cache` y `/rustsec`, y `--bind` para `/salida`. `/cache` va con `--bind` si `e.cache_rw` es verdadero;
   - `--clearenv`, y después un `--setenv` por cada variable de `entorno` y de `e.env`, en ese orden;
   - `--chdir /`, y al final `sh -c <e.cmd>`.
5. **`entorno` es el entorno de la imagen.** Una variable, `ROOT_BWRAP_ENTORNO`, apunta a un archivo con una línea `CLAVE=valor` por variable. La Tarea 141 lo escribe en el runtime. Sin el archivo, `entorno` va vacío y el modo da un aviso.
6. **Los secretos de `e.secretos`** se pasan con `--setenv` desde el proceso, nunca por un archivo. Si alguno viene en este modo, el modo da un error: en Colab no hay claves.
7. **El modo `bwrap` no pone** `--memory`, `--pids-limit` ni `--name`.

**Tests en rojo primero:**
- Unitario: una `Ejecucion` con repo, oráculo, caché y salida da los `--ro-bind` y el `--bind` en las rutas de podman.
- Unitario: con `e.red` en falso, los argumentos traen `--unshare-net`. Con `e.red` en verdadero, no.
- Unitario: los argumentos traen `--clearenv` antes de los `--setenv`, y el comando al final.
- Unitario: con `e.cache_rw` en verdadero, `/cache` va con `--bind`.
- Unitario: un secreto en modo `bwrap` da error.
- Guarda: sin la variable, `argumentos` da lo de hoy.

### 2. `ejecutar` con `bwrap`

1. **En modo `bwrap`, `ejecutar` lanza `bwrap`** con `argumentos_bwrap`, en un grupo de procesos propio (`process_group(0)`).
2. **El plazo** mata el grupo entero con `SIGKILL` y devuelve `ErrorOraculo::PlazoVencido`, como hoy. No hay contenedor que borrar por nombre.
3. **La lectura de stdout y stderr, los topes y `Resultado`** no cambian.
4. **El barrido de contenedores** no corre en modo `bwrap`. No hay contenedores.
5. **El turno de contenedor** sigue, para no lanzar dos a la vez si hoy no se lanzan.

**Tests en rojo primero:**
- Unitario: con un programa falso en vez de `bwrap`, `ejecutar` le pasa los argumentos de `argumentos_bwrap`.
- Unitario: un programa falso que duerme más que el plazo da `PlazoVencido`, y su grupo de procesos no queda vivo.
- **Un test con el `bwrap` real,** si está instalado en la laptop: corre `cat /repo/a` sobre una carpeta de solo lectura, prueba que `touch /repo/b` falla, y prueba que la red está cerrada. Se saltea si no hay `bwrap`, con un aviso como el de `saltar_sin_podman`.

### 3. Los otros lugares que llaman a podman

1. **La descarga.** En modo `bwrap`, `correr_podman_acotado` (`src/oracle/descarga.rs`) no lanza podman. Da `ErrorOraculo::Infra` con el motivo: "en modo bwrap, la caché de dependencias viene en el paquete".
2. **`digest_de_imagen`** (`src/oracle/calibra/target.rs:51`). En modo `bwrap`, devuelve el valor de la variable `ROOT_BWRAP_IMAGEN_ID`, que escribe la Tarea 141. Sin la variable, devuelve `None`.
3. **`argumentos_separado`,** el contenedor vivo de Entrega, da un error claro en modo `bwrap`.
4. **El informe lista** todo lugar con el literal `"podman"` en `src/`, y qué hace en modo `bwrap`. Usá la auditoría de la Tarea 118 como punto de partida.

**Tests en rojo primero:**
- Unitario: en modo `bwrap`, la descarga sin caché da el error con su motivo, sin lanzar podman.
- Unitario: en modo `bwrap`, `digest_de_imagen` devuelve el id de la variable.
- Unitario: en modo `bwrap`, `argumentos_separado` da error.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab ni podman. Quedan para el orquestador:
- **En Colab, después de la prueba de la 140:** una calibración de Seguridad de s2-t1 con `ROOT_CONTENEDOR_MODO=bwrap`, en el runtime preparado. En la laptop tarda 8 s.
- **En la laptop, sin la variable:** la suite con podman sigue verde.

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

Al terminar, escribí `docs/builds/informe-local-139.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los argumentos de los dos modos**: los argumentos completos de una misma `Ejecucion` con podman y con `bwrap`.
5. **Los lugares con podman**: cada uno, y qué hace en modo `bwrap`.
6. **Tests que cambiaron de valor**, con su motivo.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
