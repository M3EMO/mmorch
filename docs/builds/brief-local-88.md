# Brief de la Tarea 88 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-87` (dc4dfc7). Las Tareas 84 a 87 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-88`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, error de `try_wait` | `src/corre/ejecutor.rs` |
| 2, pánico en la limpieza | `src/corre/corrida.rs` |
| 3, `r00t oraculo capas` | `src/oracle/capas.rs` (nuevo), `src/oracle/perfiles/rust.rs` y `src/oracle/perfiles/mod.rs` (solo visibilidad y una función de grafo, si hace falta), `src/oracle/mod.rs` (declaración del módulo), `src/cli.rs` y `src/lib.rs` (el subcomando), `docs/interfaces.md` |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función o de borrar un símbolo, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

**Error de `try_wait`.** La revisión de la señal sobre dc4dfc7 encontró un MEDIUM. Si `try_wait` falla (`src/corre/ejecutor.rs:661` a `668`), Root mata el cliente `podman run` y devuelve error. No corre `podman rm` por nombre. El contenedor sigue vivo con la credencial montada. El camino del vencimiento sí la borra (línea 243), pero su `podman rm` no tiene plazo.

**Pánico en la limpieza.** Un pánico en la limpieza de la señal (`src/corre/corrida.rs:772` a `774`) mata el hilo de la señal. Root deja de salir con el código de la señal.

**Capas.** El único inyector de Arquitectura es `CicloEntreCapas` (`src/oracle/calibra/inyectar.rs:409`). Necesita 2 capas o más. En las tareas de S2, `capas = [["patron"]]` no tiene módulos reales, y el defecto nunca aplica. La regla de D3 dice que las capas salen del mapa real de módulos del repo bueno, o quedan vacías. Con capas vacías, Arquitectura nunca tiene un defecto que medir. Hace falta derivar las capas con el mismo extractor que usa la sonda. Un lector distinto puede ver otras aristas y hacer fallar el chequeo.

**Regla de la sonda.** Un módulo de una capa posterior que usa uno de una capa anterior da un hallazgo de severidad 3. Una capa anterior puede usar una posterior.

## Tarea

La regla que manda: **ningún camino de error deja un contenedor vivo con la credencial, y las capas salen del mismo lector que las juzga.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Error de `try_wait`

1. Si `try_wait` falla, Root termina el hijo, corre `podman rm` por nombre y devuelve el error. Es la misma limpieza que la del vencimiento.
2. El `podman rm` del vencimiento y el del error usan el plazo de la limpieza de la señal.
3. Un `podman rm` que vence su plazo se informa en el error. No cuelga.

**Tests en rojo primero:**
- Unitario: con la costura que fuerza el fallo de `try_wait`, el `podman` falso registra un `rm` con el nombre del contenedor.
- Unitario: un `podman rm` falso que no termina vence su plazo, y el error lo dice.
- Guarda: el camino del vencimiento sigue borrando el contenedor.

### 2. Pánico en la limpieza

1. La limpieza de la señal corre dentro de `std::panic::catch_unwind`.
2. Si hay pánico, el hilo lo escribe en stderr, borra la copia de la credencial si todavía existe, y sale con el código de la señal.

**Tests en rojo primero:**
- Unitario: con una costura que hace entrar en pánico a la limpieza, la función de salida recibe el código de la señal.
- Guarda: los tests de señal de las Tareas 84 a 87 siguen verdes.

### 3. `r00t oraculo capas`

**Proponer.** `r00t oraculo capas --repo <dir>`:
1. Lee los archivos con el mismo lector que `arquitectura_nativa`.
2. Arma el grafo de módulos con `aristas_de_capas`. Las aristas hacia la raíz no cuentan, igual que en la sonda.
3. Los módulos de un mismo ciclo van en la misma capa.
4. Cada grupo recibe su altura: 1 si no usa a ningún otro módulo, y 1 más que el mayor de los que usa.
5. Las capas van de mayor a menor altura. Dentro de una capa, los módulos van en orden alfabético.
6. Imprime una línea TOML lista para pegar: `capas = [["a"], ["b", "c"], ...]`.
7. Con `--json`, imprime también los módulos, las aristas y los ciclos.

**Verificar.** `r00t oraculo capas --repo <dir> --tarea <nombre>`:
1. Lee `capas` del `tarea.toml` de la tarea.
2. Corre la sonda nativa de Arquitectura sobre el repo, sin podman y sin API.
3. Pasa si no hay ningún hallazgo "falta la capa" ni de uso hacia una capa anterior.
4. Pasa también solo si hay 2 capas o más, y si `CicloEntreCapas` aplica sobre una copia temporal del repo.
5. La copia temporal vive dentro del árbol de trabajo de Root, no en `/tmp`, y se borra por `src/borrado.rs`.
6. Sale con 0 si pasa y con 1 si no pasa. Cada condición fallida se nombra.

**Tests en rojo primero:**
- Unitario: un repo de fixture con `a` que usa `b`, y `b` que usa `c`, propone `capas = [["a"], ["b"], ["c"]]`.
- Unitario: un ciclo entre `b` y `c` los pone en la misma capa.
- Unitario: las capas propuestas pasan la verificación sobre el mismo repo.
- Unitario: `capas = [["patron"]]` sobre un repo sin ese módulo no pasa, y nombra "falta la capa" y "menos de 2 capas".
- Unitario: un uso agregado hacia una capa anterior hace fallar la verificación.
- Guarda: la sonda de Arquitectura da los mismos hallazgos que hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-88.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
