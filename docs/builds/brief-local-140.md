# Brief de la Tarea 140 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`. Reemplaza la versión de la vuelta (53).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- **Arrancá cuando la serie 138 termine de medir.** El orquestador lo dice.
- La 139 y la 141 se construyen a la vez. La 139 toca `src/oracle/contenedor.rs`, `src/oracle/descarga.rs` y `src/oracle/calibra/target.rs`. La 141 toca `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs` y `src/oracle/colab.rs`. No toques esos archivos, ni los de la 137 y la 138.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-140`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la preparación del runtime | `tools/colab/preparar-runtime.sh` (nuevo) |
| 2, la verificación de versiones | `tools/colab/verificar-versiones.sh` (nuevo), `tests/colab_preparar.rs` (nuevo) |

- El punto 2 sirve al 1. Van en serie.
- **No toques:** `src/`, `imagenes/`, `rubricas/`, los archivos de las otras tareas y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No corras `preparar-runtime.sh` en la laptop.** Instala paquetes del sistema. Solo corre en un runtime de Colab.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

**La decisión del usuario, literal:** P110 "b) Bubblewrap". Las cuatro dimensiones de oráculo se calibran en Colab, con bubblewrap.

**En Colab no se puede construir la imagen `root-oraculo-rust:4`.** `podman build` corre sus pasos `RUN` en un contenedor, y en Colab ningún contenedor anidado corre. El runtime necesita las mismas herramientas que la imagen, instaladas a mano.

**Lo que trae la imagen** (`imagenes/oraculo-rust/Containerfile`):
- La base es `docker.io/library/rust:1`: Debian bookworm, con Rust en `/usr/local/rustup` y `/usr/local/cargo`.
- `clippy`, por `rustup component add clippy`.
- `cargo-audit` 0.22.2, `cargo-mutants` 27.1.0 y `cargo-nextest` 0.9.100, en `/usr/local/bin`.
- `gitleaks` 8.30.1 y `hadolint` 2.15.1, en `/usr/local/bin`, con su SHA-256.
- `/etc/root-oraculo/nextest.toml`, con el perfil `ci`.
- `python3` y `python3-pytest`, y las variables `CARGO_HOME=/cargo` y `CARGO_NET_OFFLINE=true`.

**Las herramientas no se compilan en el runtime. Se copian de la imagen.** La Tarea 141 las saca de la imagen en la laptop, con podman, y las sube en el paquete. Son binarios de Debian bookworm, con glibc 2.36. Colab usa Ubuntu 24.04, con glibc 2.39, así que corren. Son los mismos binarios que usa la laptop.

**La cadena de Rust sí se instala en el runtime,** con `rustup`, en la versión exacta de la imagen. La red de Google la baja rápido.

## Tarea

La regla que manda: **el runtime de Colab tiene las mismas herramientas que la imagen, y una verificación frena todo si alguna difiere.** Hacé un commit por punto. Cada punto empieza con su test en rojo, si se puede.

### 1. La preparación del runtime

`tools/colab/preparar-runtime.sh <carpeta_del_paquete>`. Corre como root en el runtime. Lee `versiones.env` de la carpeta del paquete. La Tarea 141 escribe ese archivo, con este formato:

```
RUST_VERSION=1.xx.y
RUSTUP_SHA256=<sha256 de rustup-init>
HERRAMIENTAS_SHA256=cargo-audit:<sha256>,cargo-mutants:<sha256>,cargo-nextest:<sha256>,gitleaks:<sha256>,hadolint:<sha256>
```

1. **bubblewrap** se instala con `apt-get install -y bubblewrap`.
2. **Rust** se instala con `rustup-init`, verificado por su SHA-256, en `/usr/local/rustup` y `/usr/local/cargo`, con `--default-toolchain $RUST_VERSION --profile minimal`. Después, `rustup component add clippy`. Seguí la forma de `prepara-rust.py` de `src/oracle/colab.rs`, sin tocar ese archivo.
3. **Las herramientas** se copian de `<carpeta_del_paquete>/herramientas/` a `/usr/local/bin`, con permisos `0755`.
4. **`/etc/root-oraculo/nextest.toml`** se escribe con el mismo texto que el `printf` del Containerfile. El texto vive en una sola variable del script, con un comentario que nombra la línea del Containerfile.
5. **El script termina** llamando a `verificar-versiones.sh`. Si la verificación falla, el script sale con un código distinto de 0.
6. **`set -euo pipefail`.** Cada paso imprime su nombre y su tiempo.

### 2. La verificación de versiones

`tools/colab/verificar-versiones.sh <carpeta_del_paquete>`. No instala nada. Solo lee y compara.

1. **Compara `rustc --version`** con `RUST_VERSION`.
2. **Compara `cargo clippy --version`** con la versión de la cadena: debe existir y responder.
3. **Compara el SHA-256 de cada herramienta** en `/usr/local/bin` con `HERRAMIENTAS_SHA256`.
4. **Compara el texto de `/etc/root-oraculo/nextest.toml`** con el esperado.
5. **Busca `bwrap`** y su versión.
6. **Imprime una línea por control:** `ok <control>` o `mal <control>: <esperado> contra <hallado>`. Sale con 0 solo si todos dan `ok`.
7. **Las rutas** de `/usr/local/bin` y `/etc/root-oraculo` se pueden cambiar con dos variables, para los tests.

**Tests en rojo primero, en `tests/colab_preparar.rs`:**
- Un test corre `bash -n` sobre los dos scripts.
- Un test arma una carpeta temporal con binarios falsos (`rustc`, `cargo`, `bwrap` y las herramientas), un `nextest.toml` y un `versiones.env` que coinciden. La verificación sale con 0 y solo imprime `ok`.
- Un test cambia el SHA-256 de una herramienta. La verificación sale con un código distinto de 0, y nombra la herramienta.
- Un test cambia `RUST_VERSION`. La verificación nombra a `rustc`.
- Si el sistema no tiene `bash`, los tests se saltean con un aviso.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab. Queda para el orquestador:
- **Correr `preparar-runtime.sh` en un runtime de CPU de Colab,** con un paquete de la Tarea 141. Anotar el tiempo de cada paso, las unidades, y que la verificación dé todo `ok`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes de los scripts van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-140.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El formato de `versiones.env`**: el texto exacto que esperan los scripts.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
