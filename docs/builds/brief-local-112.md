# Brief de la Tarea 112 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `claude/root-e7813e` en 64361fa. Ahí ya están mezcladas las Tareas 84 a 109.
- La Tarea 110 está en `laptop/tarea-110`, y la 111 se construye en `laptop/tarea-111`. Esta tarea no toca sus archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-112`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el dueño por tiempo | `src/limpieza.rs` |
| 2, una sola regla con `corre` | `src/corre/barrido.rs`, `src/limpieza.rs` |

- El punto 2 usa la regla del punto 1. Van en serie.
- **No toques:** `src/tipos.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs`, `src/oracle/calibra.rs`, `src/lib.rs`, el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El barrido quita contenedores vivos de otro worktree.** La suite con podman de la Tarea 110 dio 7 rojos mientras otro builder corría sus tests en otro worktree. Todos muestran contenedores matados: "podman falló (137)", "la imagen no trae sh", "podman no pudo correr el escenario config".

**La causa, en `src/limpieza.rs:106`.** `r00t_vivo` da un pid por vivo en tres casos: es este proceso, su cmdline es `r00t` o `r00t-*`, o su `/proc/<pid>/exe` es el mismo binario que el propio. Cada worktree compila su propio binario de tests, que se llama `root-<hash>`. Para el proceso A, un pid del proceso B no cumple ninguno de los tres. A lo da por muerto, y su barrido (`huerfanos`, línea 81) quita los contenedores vivos de B. Lo mismo pasa con un binario renombrado.

**Un nombre no prueba quién es el dueño de un contenedor. El tiempo sí.** El dueño es el proceso que ya existía cuando el contenedor se creó.

**La misma idea ya existe en `corre`.** `src/corre/barrido.rs` compara la hora de arranque del pid con los segundos del id de la corrida, con un margen de 300 s, por la suspensión y NTP.

## Tarea

La regla que manda: **el dueño de un contenedor vive si su pid existe y arrancó antes de que el contenedor se creara. El nombre del binario no cuenta.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El dueño por tiempo

1. **Hora de creación.** El listado de contenedores lee, además del nombre, la hora de creación de cada uno, desde `podman ps`. Elegí un formato de `podman ps` que dé la hora en segundos desde la época. Anotá el formato en el informe.
2. **Hora de arranque.** Root lee la hora de arranque del pid del nombre desde `/proc/<pid>/stat`, campo 22, y `btime` de `/proc/stat`. Es la misma lectura que ya hace `src/corre/barrido.rs`.
3. **La regla.** El dueño está vivo si el pid existe y arrancó antes de la creación del contenedor más 300 s.
4. **Muerto:** el pid no existe, o arrancó después de la creación más 300 s.
5. **Sin hora:** si no se puede leer la hora de creación o la de arranque, el dueño cuenta como vivo, y el contenedor se conserva. El error queda del lado de no borrar.
6. `r00t_vivo` deja de mirar el nombre y el binario. Si `es_r00t` y `mismo_binario` quedan sin uso, borralos después de unir sus llamadores.
7. La raíz de `/proc` sigue inyectable, para los tests.

**Tests en rojo primero:**
- Unitario: un contenedor cuyo pid es un proceso vivo llamado `root-abc123`, que arrancó antes de la creación, se conserva. Hoy se quita.
- Unitario: un contenedor cuyo pid es un proceso vivo que arrancó 400 s después de la creación se quita.
- Unitario: un contenedor cuyo pid no existe se quita.
- Unitario: un contenedor sin hora de creación legible se conserva.
- Guarda: el pid propio se conserva.
- Guarda: los tests de barrido de la 107 y la 107b siguen verdes, salvo los que fijaban la regla del nombre. Anotá esos en el informe.

### 2. Una sola regla con `corre`

1. `src/corre/barrido.rs` usa la regla de `src/limpieza.rs`. La hora de referencia es la de los segundos del id de la corrida, en vez de la creación de un contenedor.
2. Una sola función recibe el pid, la hora de referencia y la raíz de `/proc`, y dice si el dueño vive.
3. El margen de 300 s queda en una sola constante, con su motivo en el comentario.
4. Borrá la lectura duplicada de la hora de arranque en `corre::barrido`, después de unir sus llamadores.

**Tests en rojo primero:**
- Unitario: una corrida de un proceso vivo llamado `root-abc123`, que arrancó antes de los segundos del id, no se barre.
- Guarda: los tests de barrido de `corre` de las Tareas 84, 85, 87 y 108 siguen verdes, incluido el margen de 300 s.

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

Al terminar, escribí `docs/builds/informe-local-112.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Formato de la hora de creación**: el formato de `podman ps` que se usa, y su fuente.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
