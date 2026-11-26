# Brief de la Tarea 174 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (101).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-173b`** (8eb25e1). Trae 5a4f295, la 171, la 172, la 173 y la 173b. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-174`.
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

**La prueba de aislamiento corrió en Colab y frenó en `preparar-corre.sh`,** en el paso `versiones`, con `rustc: command not found`. Al revisar ese fallo, aparecieron cuatro defectos. Los cuatro impiden que el builder de `corre` trabaje en Colab.

1. **`preparar-corre.sh` no ve Rust.** `preparar-runtime.sh` instala Rust con `RUSTUP_HOME=/usr/local/rustup` y `CARGO_HOME=/usr/local/cargo`, y suma `/usr/local/cargo/bin` a su `PATH`. Pero corre como un guion aparte. Ese entorno no vuelve a `preparar-corre.sh`.
2. **El builder no ve Rust.** En `src/corre/bwrap.rs`, el builder corre con `env_clear()` y `PATH_FIJO=/usr/local/bin:/usr/bin:/bin`. `cargo` y `rustc` viven en `/usr/local/cargo/bin`. Además, sin `RUSTUP_HOME`, los proxies de rustup buscan el toolchain en `~/.rustup`, que no existe. Y `cargo` necesita un `CARGO_HOME` escribible, pero `/usr` está montado en solo lectura. **Sin este arreglo, el builder de Colab no puede correr `cargo test`, y D3 mediría corridas sin tests.**
3. **`node` no es el de la imagen del builder.** `preparar-corre.sh` instala `nodejs` con apt, y da la v18.19.1. `@anthropic-ai/claude-code@2.1.317` pide node >=22: npm avisó `EBADENGINE`. `claude --version` pasó, pero eso no prueba que el resto ande. En la laptop, `corre` corre con el `node` de la imagen `root-builder:1`, y ese anda.
4. **El builder no puede entrar a su carpeta de corrida.** `corre` arma la corrida en `$HOME/.cache/root/corridas/<id>/`, con su `repo/` y su `claude-estado/`. En Colab, `r00t` corre como root con `HOME=/root`, y `/root` tiene modo 0700. bwrap corre como el usuario `builder`, después de `setpriv`, así que no puede montar esas carpetas. Y aunque pudiera, son de root: el builder no podría escribir en `/trabajo` ni en su estado.
   - **Un efecto más:** cuando el builder deja archivos suyos en `repo/`, el `git` de `r00t`, como root, falla con "dubious ownership".

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, Rust en `preparar-corre.sh` | `tools/colab/preparar-corre.sh` |
| 2, Rust en el builder | `src/corre/bwrap.rs` |
| 3, el `node` de la imagen | `src/colab_corre.rs`, `tools/colab/preparar-corre.sh` |
| 4, la carpeta de corrida del builder | `src/corre/bwrap.rs`, `tools/colab/preparar-corre.sh`, `tools/colab/corre-runtime.py` |

- **Los tests de los guiones viven en `src/colab_corre.rs`,** como en la 172 y la 173.
- Los puntos comparten archivos. Van en serie: 1, 2, 3 y 4.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/colab/preparar-runtime.sh`, `tools/colab/red-builder.sh`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Ninguna huella cambia.**
- **Ningún montaje nuevo en bwrap.** El builder sigue viendo solo su repo, su estado y el plugin. Así, lo que pruebe la prueba de aislamiento vale para esta versión.
- **Sin cambio de esquema,** y **sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.** Los tests usan programas falsos.
- **Ningún test cambia el dueño de un archivo a otro usuario, ni crea un usuario.** Eso pide root. Los tests usan un `chown` falso, o revisan la llamada.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **en Colab, el builder tiene las mismas herramientas que en la imagen `root-builder:1`, y trabaja en carpetas suyas, sin ver nada más.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Rust en `preparar-corre.sh`

1. **Después de `preparar-runtime.sh`,** `preparar-corre.sh` exporta `RUSTUP_HOME=/usr/local/rustup`, `CARGO_HOME=/usr/local/cargo` y `PATH=/usr/local/cargo/bin:$PATH`. Son los mismos valores de `preparar-runtime.sh`.
2. **Un test guarda que los valores son los mismos.** Lee los dos guiones y compara los literales de `RUSTUP_HOME` y `CARGO_HOME`.
3. **El paso `versiones`** encuentra `rustc` y compara su versión con `versiones-builder.env`, como hoy.

**Tests en rojo primero, con comandos falsos:**
- Unitario: con un `rustc` falso solo en `<raíz>/usr/local/cargo/bin`, el paso `versiones` pasa. Hoy da `rustc: command not found`.
- Unitario: los literales de `RUSTUP_HOME` y `CARGO_HOME` de `preparar-corre.sh` son iguales a los de `preparar-runtime.sh`.

### 2. Rust en el builder

1. **`PATH_FIJO` pasa a `/usr/local/cargo/bin:/usr/local/bin:/usr/bin:/bin`.**
2. **El entorno del builder suma `RUSTUP_HOME=/usr/local/rustup`.** Esa carpeta queda en solo lectura, dentro de `/usr`.
3. **El entorno del builder suma `CARGO_HOME=/home/builder/.claude/cargo-home`.** Es una carpeta dentro del estado, que ya está montado con escritura. No hace falta otro montaje.
4. **Root crea `cargo-home/` dentro del estado** antes de cada lanzamiento, con el mismo dueño que el estado.
5. **El valor por defecto de `ROOT_PATH_BUILDER` en `preparar-corre.sh` es igual a `PATH_FIJO`.** Un test lo guarda.
6. **El paso `versiones` suma una revisión con el entorno del builder.** Como el usuario `builder`, con `setpriv` y un entorno limpio con el `PATH`, `RUSTUP_HOME` y un `CARGO_HOME` temporal del builder: `cargo --version`, `rustc --version`, `node --version`, `claude --version` y `git --version` andan. Un fallo nombra el comando.

**Tests en rojo primero:**
- Unitario: el entorno del `Command` trae `PATH` con `/usr/local/cargo/bin` primero, `RUSTUP_HOME` y `CARGO_HOME`. Hoy no trae ni `RUSTUP_HOME` ni `CARGO_HOME`.
- Unitario: los montajes de bwrap son los mismos que en la base. Ningún montaje nuevo.
- Unitario: antes de lanzar, existe `cargo-home/` dentro del estado.
- Unitario: el `PATH` por defecto de `preparar-corre.sh` es igual a `PATH_FIJO`.
- Unitario de guion, con `setpriv` y comandos falsos: si `cargo --version` falla con el entorno del builder, el guion frena y lo nombra.
- Guarda: las variables prohibidas siguen fuera del entorno.

### 3. El `node` de la imagen

1. **`versiones-builder.env` suma `NODE_VERSION`.** `colab empaquetar --corre` la lee de la imagen `root-builder:1` con `node --version`, como ya lee `claude` y `rustc`.
2. **`preparar-corre.sh` instala esa versión exacta de `node`** desde `https://nodejs.org/dist/v<versión>/node-v<versión>-linux-x64.tar.xz`, bajo `/usr/local`.
   - Baja también `SHASUMS256.txt` de la misma carpeta, y revisa el SHA-256 del archivo. Si no coincide, frena.
   - Saca `nodejs` y `npm` de la lista de apt.
   - Instala `claude` con el `npm` de ese `node`, con `--prefix /usr/local`.
3. **Las revisiones de la 173 siguen:** `claude` y `node` quedan bajo `/usr/`. Suma una más: `node --version` es igual a `NODE_VERSION`.
4. **Sin `NODE_VERSION` en `versiones-builder.env`,** el guion frena y lo nombra.

**Tests en rojo primero:**
- Unitario: el paquete de `corre` trae `NODE_VERSION` en `versiones-builder.env`. Hoy no lo trae.
- Unitario de guion, con `curl` y `node` falsos: un SHA-256 distinto frena el guion. Hoy el guion no baja `node`.
- Unitario de guion: un `node --version` distinto de `NODE_VERSION` frena el guion.
- Unitario de guion: sin `NODE_VERSION`, el guion frena.

### 4. La carpeta de corrida del builder

1. **`preparar-corre.sh` crea `/srv/root-usuario`, de root, con modo 0711.** También crea `/srv/root-usuario/.cache/root/corridas`, con modo 0711 en cada nivel. El builder puede pasar por esas carpetas, pero no listarlas.
2. **`corre-runtime.py` corre el `r00t` hijo con `HOME=/srv/root-usuario`.** Así, las carpetas de corrida quedan fuera de `/root`.
3. **`preparar-corre.sh` fija `git config --system --add safe.directory '*'`,** como `builder-colab.sh` en sus runtimes. Así, el `git` de `r00t` puede leer un repo del builder.
4. **Antes de cada lanzamiento, `EjecutorBwrapBuilder` cambia el dueño de `repo/` y del estado al usuario `builder`, en forma recursiva.** No sigue enlaces simbólicos.
5. **El chequeo previo suma una revisión:** cada carpeta que contiene a `repo/` y al estado deja pasar al builder. Si una no lo deja, es un error de infraestructura que nombra la carpeta.
6. **El chequeo de la 171 sigue:** el `$HOME` de `r00t` no es legible por el builder. Con 0711, el builder pasa, pero no lista.

**Tests en rojo primero:**
- Unitario, con un `chown` falso o con la llamada registrada: antes del lanzamiento, Root pide cambiar el dueño de `repo/` y del estado al uid del builder. Hoy no lo hace.
- Unitario: una carpeta de corrida dentro de una carpeta con modo 0700 frena el lanzamiento, y el error nombra esa carpeta.
- Unitario: con modo 0711 en cada nivel, el chequeo pasa.
- Unitario de Python: el entorno del `r00t` hijo trae `HOME=/srv/root-usuario`.
- Unitario de guion: `preparar-corre.sh` crea `/srv/root-usuario` y sus niveles con modo 0711, y fija `safe.directory`.
- Guarda: los tests de la 171 sobre `$HOME` legible siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/cadena-174`, que es la cadena 173b más esta tarea.
2. **`git diff laptop/cadena-173b laptop/tarea-174 --stat`** toca solo los archivos de la tabla y el informe.
3. **Los montajes de bwrap no cambian:** el diff de `args_bwrap_builder` no suma ningún `--bind` ni `--ro-bind`.
4. **Un paquete real de `corre`** trae `NODE_VERSION`, y su valor es el de `podman run --rm root-builder:1 node --version`.
5. **La prueba de aislamiento en Colab,** con el release de `laptop/cadena-174`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo del paquete. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-174.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El entorno y las carpetas del builder**: cada variable, cada carpeta y su dueño y modo, en la laptop y en Colab.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
