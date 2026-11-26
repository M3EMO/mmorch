# Brief de la Tarea 175 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (102).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-174`** (f7fd89b). Trae 5a4f295 y las Tareas 171 a 174. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-175`.
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

**`red-builder.sh` falla en Colab con rc=4.** El runtime de Colab es un contenedor Docker sin `CAP_NET_ADMIN`, y con `/proc/sys` en solo lectura. Por eso:
- `sysctl -w net.ipv6.conf.all.disable_ipv6=1` da "Read-only file system";
- `iptables` e `ip6tables` dan "Permission denied", también dentro de `ip netns exec`, porque `ip netns add` crea un espacio de red del mismo espacio de usuario del contenedor.

**Las sondas del orquestador en Colab mostraron un camino que anda:**
- un espacio de red creado con `unshare -Urn` pertenece a un espacio de usuario nuevo, y root tiene ahí todas las capacidades. Dentro, `iptables` e `ip6tables` andan;
- `/dev/net/tun` no existe, pero `mknod /dev/net/tun c 10 200` anda;
- `slirp4netns --configure --disable-host-loopback --mtu=65520 --userns-path=/proc/<pid>/ns/user <pid> tap0` anda;
- el espacio nuevo no tiene direcciones ni rutas IPv6, y `ip6tables -P OUTPUT DROP` queda puesto;
- `/etc/resolv.conf` de Colab apunta a 127.0.0.11, que no existe en el espacio nuevo. Con `nameserver 10.0.2.3`, el DNS anda;
- con las reglas puestas, 169.254.169.254 y 10.0.2.2 no conectan.

## La regla de seguridad que no cambia

**El builder entra solo al espacio de red, nunca al espacio de usuario dueño de ese espacio.** Hoy `src/corre/bwrap.rs` lanza `ip netns exec <red> prlimit … setpriv --reuid=<builder> … bwrap …`. Ese orden queda igual. Así:
- **el builder sigue siendo el usuario `builder` del host.** No es root mapeado. Los permisos de archivo de la capa 1 siguen valiendo;
- **el builder no tiene capacidades sobre el espacio de red.** El dueño es un espacio de usuario de root, y el builder no puede entrar a él. No puede borrar las reglas.

**Lo que no hay que hacer:** entrar al builder con `nsenter -U`. En la sonda, eso dejó al proceso como uid 0 real del host, con un uid 1000 solo de nombre dentro de bwrap.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1 a 5, la red del builder | `tools/colab/red-builder.sh`, `tests/red_builder.rs` |

- **No toques `src/`.** El lanzamiento del builder no cambia.
- **Ningún montaje nuevo en bwrap.** El `resolv.conf` del builder llega por `/etc/netns/<red>/resolv.conf`, que `ip netns exec` monta sobre `/etc/resolv.conf`. bwrap monta `/etc` desde ese mismo espacio de montaje.
- **No toques:** `src/`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Ninguna huella cambia.** **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **Ningún test crea un espacio de red o de usuario, ni corre `iptables`, `ip6tables`, `unshare`, `nsenter`, `slirp4netns`, `mknod` o `ip` reales.** Los tests usan programas falsos que anotan sus argumentos, como en la 171 y la 173.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá con `grep -rn`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **el espacio de red del builder pertenece a un espacio de usuario de root, sale a Internet solo por slirp4netns, y el builder entra solo al espacio de red.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El espacio de red, dueño de un espacio de usuario de root

1. **El sostén:** `unshare -Urn sleep infinity`, en segundo plano. Su pid va a `ROOT_SOSTEN_PID`, como hoy.
2. **El nombre:** `ip netns attach <red> <pid del sostén>`. Así, `ip netns exec <red>` y `/run/netns/<red>` siguen valiendo para `src/corre/bwrap.rs` y para `argv.py`.
3. **Idempotente:**
   - con `/run/netns/<red>` y el sostén vivo, no hace nada;
   - con `/run/netns/<red>` y el sostén muerto, corre `ip netns delete <red>`, y crea el espacio de nuevo.
4. **Sale `ip netns add`.**
5. **Antes de crear nada,** revisa que existan `unshare`, `nsenter`, `slirp4netns`, `mknod`, `iptables` e `ip6tables`. Si falta uno, sale con 1 y lo nombra.

**Tests en rojo primero, con programas falsos:**
- Unitario: el guion llama a `unshare -Urn` y después a `ip netns attach`, con el pid del sostén. Hoy llama a `ip netns add`.
- Unitario: con el sostén vivo y el nombre puesto, no crea nada.
- Unitario: con el nombre puesto y el sostén muerto, borra el nombre y crea el espacio otra vez.
- Unitario: sin `nsenter`, sale con 1 y lo nombra.

### 2. `/dev/net/tun` y slirp4netns

1. **Si `/dev/net/tun` no existe,** el guion crea `/dev/net/` y corre `mknod /dev/net/tun c 10 200`, con modo 0666. Es el modo de siempre en Linux: abrirlo no da permiso para crear una interfaz.
2. **Si ya existe, no lo toca.**
3. **slirp4netns** suma `--userns-path=/proc/<pid del sostén>/ns/user`. El resto de los argumentos queda igual, con su log en `ROOT_SLIRP_LOG`.

**Tests en rojo primero:**
- Unitario: sin `/dev/net/tun`, el guion lo crea con `c 10 200`. En el test, la ruta es un temporal del worktree.
- Unitario: con `/dev/net/tun`, no llama a `mknod`.
- Unitario: slirp4netns recibe `--userns-path=/proc/<pid>/ns/user`.

### 3. Las reglas, desde el espacio de usuario dueño

1. **Toda regla corre con `nsenter -t <pid del sostén> -U -n`,** no con `ip netns exec`. Solo root entra así.
2. **IPv4 no cambia:** 2 reglas `ACCEPT` al DNS de slirp, 10.0.2.3, por UDP y TCP 53, y después las 6 reglas `DROP` de hoy, con `! -o lo`.
3. **IPv6:** `ip6tables -P OUTPUT DROP`, `ip6tables -P INPUT DROP` y `ip6tables -P FORWARD DROP`.
4. **Sale el `sysctl` de `disable_ipv6`.** En Colab no se puede escribir.
5. **Una revisión al final:** con `nsenter -t <pid> -U -n`, `ip -6 route show default` sale vacío, e `ip -6 addr show scope global` sale vacío. Si no, el guion frena y lo dice.

**Tests en rojo primero:**
- Unitario: ninguna llamada a `sysctl`. Hoy hay dos.
- Unitario: cada llamada a `iptables` e `ip6tables` va detrás de `nsenter -t <pid> -U -n`.
- Unitario: las 3 políticas de `ip6tables` quedan en `DROP`.
- Unitario: un `ip` falso que muestra una ruta IPv6 por defecto frena el guion.
- Guarda: el orden de las reglas de IPv4 es el de la 171: DNS primero, después los 6 cortes.

### 4. El `resolv.conf` del builder

1. **El guion escribe `/etc/netns/<red>/resolv.conf`,** con una sola línea: `nameserver 10.0.2.3`. Modo 0644, dueño root.
2. **No toca `/etc/resolv.conf` del runtime.** `r00t` y la CLI de Colab lo necesitan.
3. **`ip netns exec <red>` monta ese archivo sobre `/etc/resolv.conf`** en su espacio de montaje. bwrap monta `/etc` desde ahí. Por eso no hace falta otro montaje.

**Tests en rojo primero:**
- Unitario: el guion escribe `etc/netns/<red>/resolv.conf` con `nameserver 10.0.2.3`. En el test, la raíz es un temporal.
- Unitario: `/etc/resolv.conf` de la raíz del test no cambia.

### 5. La comprobación

1. **`comprobar_red` queda igual:** `ip netns exec <red> getent hosts api.anthropic.com`, con sus reintentos. Ahora prueba también el `resolv.conf` del punto 4.
2. **Termina con `red lista`.**

**Tests:**
- Guarda: los tests de `tests/red_builder.rs` que no hablan de `ip netns add`, `sysctl` ni las reglas siguen verdes. Los que sí, cambian, y el informe los nombra.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **Antes de lanzar este builder: la sonda 6 en Colab,** de la vuelta 102, sección 3. Si la sonda falla, este brief vuelve al planner.
2. **La suite con podman** de `laptop/cadena-175`, que es la cadena 174 más esta tarea.
3. **`git diff laptop/cadena-174 laptop/tarea-175 --stat`** toca solo `tools/colab/red-builder.sh`, `tests/red_builder.rs` y el informe.
4. **La prueba de aislamiento en Colab,** con el release de `laptop/cadena-175`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-175.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La red, paso a paso**: cada comando del guion, en orden, con el espacio donde corre.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
