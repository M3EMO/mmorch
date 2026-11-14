# Brief de la Tarea 122 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-120`. Trae las Tareas 84 a 120.
- La 121 se construye a la vez. Toca `src/oracle/calibra.rs`, `src/oracle/calibra/inestables.rs` y `docs/interfaces.md`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-122`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la carrera en `corrida.rs` | `src/corre/corrida.rs`, solo su módulo de tests |
| 2, la carrera en `tests/corre.rs` | `tests/corre.rs` |
| 3, la lista vacía | `src/limpieza.rs`, solo su módulo de tests |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **Solo tests.** Ningún punto cambia código de producción.
- **No toques:** el código de producción de cualquier archivo, `src/oracle/`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La carrera ETXTBSY.** Un helper de tests escribe un script falso. Mientras tanto, otro hilo hace fork, y el hijo hereda el descriptor de escritura. Ese descriptor recién se cierra cuando el hijo hace `exec`. Si el test lanza el script en ese momento, el kernel da ETXTBSY, el código 26 de Linux. El hook corre la suite en cada commit, y una falla así frena el commit.

**La Tarea 120 la arregló en `src/limpieza.rs`.** Su `podman_falso` lanza el script con `--sonda`, con reintentos cortos, antes de devolverlo. Si una prueba anda, ningún hijo guarda el descriptor. Los forks nuevos ya no lo heredan.

**Quedan dos helpers con la misma carrera,** según el informe de la 120:
- `podman_falso` de `src/corre/corrida.rs:3778`, en su módulo de tests.
- `podman_falso` de `tests/corre.rs:233`.

**La lista vacía.** El `podman_falso` de `src/limpieza.rs` imprime un error de sintaxis de `sh` cuando recibe una lista vacía. No cambia los resultados. Pero un error impreso en cada corrida tapa los errores reales.

## Tarea

La regla que manda: **un helper de tests devuelve un script que ya se puede lanzar, y no imprime errores falsos.** Hacé un commit por punto. Cada punto empieza con su test en rojo, si se puede reproducir.

### 1. La carrera en `corrida.rs`

1. El `podman_falso` de `src/corre/corrida.rs` prueba el script antes de devolverlo, como el de `src/limpieza.rs` desde la 120.
2. La prueba lanza el script con `--sonda`. Con el código 26 (`raw_os_error() == Some(26)`), espera 10 ms y prueba otra vez, hasta 50 veces.
3. Con `--sonda`, el script sale con 0 y no deja marcas.
4. Anotá el motivo del 26 en un comentario.

**Tests en rojo primero:**
- Unitario: 200 repeticiones del helper mientras otro hilo hace fork en bucle no fallan. Si el rojo no se reproduce, anotalo, y dejá la repetición como guarda.

### 2. La carrera en `tests/corre.rs`

1. El `podman_falso` de `tests/corre.rs` sigue la misma regla del punto 1.
2. `tests/` no ve los helpers de `src/`. Copiá la prueba en el archivo, con un comentario que nombre el original.

**Tests en rojo primero:**
- Igual que el punto 1, en `tests/corre.rs`.

### 3. La lista vacía

1. El `podman_falso` de `src/limpieza.rs` escribe un script válido con una lista vacía.
2. Con la lista vacía, el script imprime nada para `ps`, y sale con 0.

**Tests en rojo primero:**
- Unitario: el script con la lista vacía corre con `sh -n` sin error. Hoy da un error de sintaxis.
- Unitario: el script con la lista vacía, llamado con `ps`, no escribe nada en stderr.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

Ninguno. Esta tarea solo toca helpers de tests sin podman. El orquestador corre la suite con podman al mezclar, como siempre.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-122.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El rojo de la carrera**: si se reprodujo, y en cuántas de las 200 repeticiones.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
