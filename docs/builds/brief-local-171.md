# Brief de la Tarea 171 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (98).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde 5a4f295, en `claude/root-e7813e`.** Es el congelado Y.
- Trabajá en una rama nueva: `laptop/tarea-171`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- **La Tarea 172 puede correr a la vez, en otro runtime.** No comparten archivos. Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Por qué

**El usuario eligió correr D3 en Colab (P129 b).** D3 corre `r00t corre` sobre 31 tareas. Hoy el builder de `corre` corre en un contenedor de podman (`src/corre/contenedor.rs`, `EjecutorContenedorBuilder`). Colab no tiene podman.

**El contenedor es lo que separa al builder de los tests ocultos.** El builder ve su carpeta de corrida, su estado y su credencial. Nunca ve `ROOT_HOME` ni `$HOME`. En Colab, el `ROOT_HOME`, con los tests ocultos, vive en el mismo runtime que el builder. Esta tarea da la misma separación sin podman.

**Colab suma dos riesgos que la laptop no tiene:**
- **El kernel de Colab escucha en la red local del runtime, y corre como root.** Un proceso que llega a ese puerto puede correr código como root, y leer `ROOT_HOME`.
- **El servidor de metadatos de la VM** responde en 169.254.169.254.

## La separación, en cuatro capas

1. **Un usuario aparte.** `r00t` corre como root. El `ROOT_HOME` es de root, con modo 0700. El builder corre como el usuario `builder`, sin permiso de lectura sobre `ROOT_HOME`.
2. **bwrap.** El builder corre dentro de bwrap, con sus propios espacios de nombres de usuario, procesos, IPC y UTS. Ve el sistema en solo lectura, y solo tres carpetas con escritura o lectura propias.
3. **Una red aparte.** El builder corre en un espacio de red propio, `root-builder`. Ese espacio sale a Internet por `slirp4netns`, sin acceso a la red local del runtime. Una regla de `iptables` corta la red local y la de metadatos.
4. **Sin privilegios nuevos.** `setpriv` quita las capacidades y fija `no_new_privs`. Un binario `setuid` no le da root.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el ejecutor del builder con bwrap | `src/corre/ejecutor.rs`, `src/corre/contenedor.rs`, un módulo nuevo `src/corre/bwrap.rs`, `src/corre/mod.rs` |
| 2, la elección del ejecutor | `src/corre/corrida.rs`, `src/corre/mod.rs` |
| 3, la red del builder | un guion nuevo `tools/colab/red-builder.sh` |

- Los puntos van en serie: 1, 2 y 3.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/colab_calibra.rs`, `src/cli.rs`, `src/tipos.rs`, el resto de `src/`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`. La Tarea 172 toca `src/cli.rs`, `src/lib.rs` y `tools/colab/` salvo `red-builder.sh`.
- **Ninguna huella cambia.** Esta tarea no toca lo que mide el oráculo.
- **Sin cambio de esquema,** y **sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.** Los tests usan programas falsos.
- **Los tests con bwrap real** se saltan con un aviso si `bwrap` no está, o si el sistema no permite espacios de usuario. Usá la misma forma que los tests de `ROOT_SIN_PODMAN`.
- **Ningún test crea un espacio de red ni corre `iptables`.** Eso pide root. Lo prueba el orquestador en Colab.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **en bwrap, el builder ve lo mismo que en podman, y nada más: su carpeta de corrida, su estado, su credencial y el plugin.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El ejecutor del builder con bwrap

1. **`EjecutorBwrapBuilder` implementa `EjecutorBuilder`,** como `EjecutorContenedorBuilder`. Reusa `env_del_builder`, `validar_montajes`, `preparar_credencial` y `VARIABLES_PROHIBIDAS`.
2. **El comando es, en este orden:**
   `ip netns exec <red> setpriv --reuid=<uid> --regid=<gid> --clear-groups --inh-caps=-all --bounding-set=-all --no-new-privs bwrap <argumentos> claude <args del lanzamiento>`.
   - `<red>` sale de la variable `ROOT_RED_BUILDER`.
   - `<uid>` y `<gid>` salen del usuario de la variable `ROOT_USUARIO_BUILDER`.
3. **Los argumentos de bwrap:**
   - `--unshare-user`, `--unshare-pid`, `--unshare-ipc`, `--unshare-uts`, `--unshare-cgroup-try`, `--die-with-parent` y `--new-session`;
   - `--ro-bind` de `/usr`, `/etc`, `/opt` si existe, y los enlaces `/bin`, `/lib`, `/lib64` y `/sbin` como en el sistema;
   - `--proc /proc`, `--dev /dev` y `--tmpfs /tmp`;
   - `--bind <repo> /trabajo`, `--bind <estado> /home/builder/.claude` y `--ro-bind <copia de la credencial>` en el mismo destino que hoy;
   - `--ro-bind <plugin>` en `PLUGIN_EN_CONTENEDOR`, si el lanzamiento trae plugin;
   - `--chdir /trabajo`.
4. **Nunca monta** `/root`, `/content`, `/home` del sistema, `ROOT_HOME` ni el `$HOME` de quien corre `r00t`. `validar_montajes` lo revisa antes de lanzar.
5. **El entorno no va en los argumentos.** El `Command` empieza con `env_clear()`, y suma solo `env_del_builder`, `HOME=/home/builder` y un `PATH` fijo. bwrap lo hereda. Ningún valor del entorno aparece en `argv`.
6. **La credencial por token.** Si la variable `ROOT_TOKEN_BUILDER` nombra un archivo, Root lo lee y lo pasa al builder como `CLAUDE_CODE_OAUTH_TOKEN`, por el entorno. Nunca lo imprime, nunca lo escribe en otro archivo, y nunca lo pone en un argumento. Sin esa variable, usa la copia de `.credentials.json`, como hoy.
7. **El chequeo previo, antes de cada lanzamiento.** Si falla, es un error de infraestructura con su motivo:
   - `ROOT_RED_BUILDER` y `ROOT_USUARIO_BUILDER` existen;
   - el uid del builder no es el de quien corre `r00t`;
   - `ROOT_HOME` es de quien corre `r00t`, con modo 0700 o más cerrado;
   - el `$HOME` de quien corre `r00t` no es legible por el builder.
8. **El plazo.** `TIMEOUT_BUILDER` vale igual. Al vencer, Root mata el grupo de procesos entero.
9. **Los fallos de bwrap, de `setpriv` o de `ip netns`** son errores de infraestructura, como hoy los de podman. Una salida de `claude` nunca se confunde con uno de esos fallos.
10. **Los topes que no hay sin cgroups.** La memoria no tiene tope. El número de procesos usa `RLIMIT_NPROC` de 4096, por `prlimit`. El informe lo anota como diferencia con podman.

**Tests en rojo primero:**
- Unitario: los argumentos de bwrap no montan `ROOT_HOME`, ni `$HOME`, ni `/root`, ni `/content`. Hoy `EjecutorBwrapBuilder` no existe.
- Unitario: con `ROOT_TOKEN_BUILDER`, el valor del token no aparece en ningún argumento. Sí aparece en el entorno del `Command`.
- Unitario: el entorno del `Command` no trae `ROOT_HOME`, `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`, aunque el proceso las tenga.
- Unitario: un `ROOT_HOME` con modo 0755 frena el lanzamiento, con un error de infraestructura.
- Unitario: un builder con el mismo uid de `r00t` frena el lanzamiento.
- Unitario: un fallo de bwrap da un error de infraestructura, no una salida del builder.
- Integración con bwrap real, sin red ni `setpriv`: un programa falso, en lugar de `claude`, intenta leer un archivo canario fuera de los montajes. No lo lee. El mismo programa, fuera de bwrap, sí lo lee.
- Integración con bwrap real: el programa falso escribe en `/trabajo`, y el archivo aparece en la carpeta de la corrida. Escribir en `/usr` falla.
- Guarda: los tests de `EjecutorContenedorBuilder` siguen verdes.

### 2. La elección del ejecutor

1. **Con `ROOT_CONTENEDOR_MODO=bwrap`,** `corre` usa `EjecutorBwrapBuilder`. Con otro valor, o sin la variable, usa `EjecutorContenedorBuilder`, como hoy.
2. **La variable es la misma del oráculo.** Así, en Colab, el oráculo y el builder usan bwrap juntos.
3. **El veredicto de la corrida registra el ejecutor:** un aviso dice "builder en bwrap" cuando corresponde.

**Tests en rojo primero:**
- Unitario: con `ROOT_CONTENEDOR_MODO=bwrap`, la corrida arma `EjecutorBwrapBuilder`. Hoy arma el de podman.
- Unitario: sin la variable, la corrida arma `EjecutorContenedorBuilder`.
- Unitario: con bwrap, el veredicto trae el aviso "builder en bwrap".

### 3. La red del builder

`tools/colab/red-builder.sh` corre como root, en Colab, antes de la primera corrida. Debe ser idempotente.
1. Crea el espacio de red `root-builder`, si no existe.
2. Lanza `slirp4netns --configure --disable-host-loopback --mtu=65520` sobre ese espacio, y guarda su pid en `/run/root-builder-slirp.pid`.
3. Dentro del espacio, agrega reglas de `iptables` de salida:
   - acepta el DNS de slirp, 10.0.2.3, por UDP y TCP 53;
   - corta 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, 169.254.0.0/16 y 127.0.0.0/8 fuera de `lo`.
4. Termina con `red lista` si `getent hosts api.anthropic.com` responde dentro del espacio.
5. Ningún dato secreto pasa por el guion.

**Tests en rojo primero:**
- Unitario, con un `ip` y un `iptables` falsos en el `PATH`: el guion pide las 7 reglas de corte, y la de DNS va primero.
- Unitario: corrido dos veces, no crea el espacio dos veces.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/tarea-171`. Las corridas de `corre` con podman siguen iguales.
2. **`git diff 5a4f295 laptop/tarea-171 --stat`** toca solo los archivos de la tabla y el informe.
3. **La prueba de aislamiento en Colab,** de la sección 4 de la vuelta 98, cuando estén las dos tareas.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo nuevo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-171.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`. Decí cuántos tests de bwrap se saltaron y por qué.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Podman y bwrap, lado a lado**: cada montaje, variable, tope y red, en los dos ejecutores.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
