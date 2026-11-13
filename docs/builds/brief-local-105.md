# Brief de la Tarea 105 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-104` (3b48385). Trae la 104 y la 104b, sobre la 103. Las Tareas 84 a 104 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-105`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, una sola lista de fuentes de secretos | `src/oracle/previos.rs`; en `src/oracle/juez.rs`, `src/oracle/configs.rs` y `src/oracle/descarga.rs`, solo la visibilidad de su constante de sonda, o una constante nueva para `"descarga"` |
| 2, la regla de `ENV` o `ARG` | `src/oracle/configs.rs` |
| 3, un test por emisor | `src/oracle/previos.rs`, o un archivo nuevo en `tests/` |

- Los puntos 1 y 2 tocan `configs.rs`. Van en serie: 1, 2 y 3.
- **No toques:** el resto de `src/`, los tests existentes salvo literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P96, literal:** "a". Incluye "Un secreto bloquea siempre."

**Hoy un secreto se reconoce por su fuente.** `es_fuente_de_secretos` (`src/oracle/previos.rs:273`) mira si la fuente es `gitleaks` o `configs-secretos`, con la lista `FUENTES_DE_SECRETOS` (línea 270). Hay tres problemas:

1. **La lista repite literales.** Las constantes existen: `SONDA_GITLEAKS` en `src/oracle/juez.rs:36` y `SONDA_SECRETOS` en `src/oracle/configs.rs:18`. Si cambia un nombre allí, el bloqueo se rompe sin que falle un test.
2. **La regla "secreto literal en ENV o ARG"** de `reglas_dockerfile` (`src/oracle/configs.rs:657`) emite con la fuente `dockerfile`. `es_fuente_de_secretos` no la reconoce. Un secreto previo en un `Dockerfile` sale como aviso, y no bloquea.
3. **La descarga** da un hallazgo de Seguridad de severidad 3 por cada secreto escrito en el repo (`src/oracle/descarga/inventario.rs:983`). Su fuente es el literal `"descarga"` (`src/oracle/descarga.rs:276`). Hoy no entra a la línea base, así que bloquea siempre. Pero si alguien la agrega a la línea base, deja de bloquear sin que falle nada.

## Tarea

La regla que manda: **un secreto bloquea siempre, venga de la sonda que venga, y la lista de fuentes no se puede desincronizar en silencio.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo donde se dice.

### 1. Una sola lista de fuentes de secretos

1. `FUENTES_DE_SECRETOS` se arma con las constantes de cada sonda, no con literales: la de gitleaks, la de `configs-secretos` y la de la descarga.
2. Si hace falta, esas constantes pasan a `pub(crate)`. Para la descarga, creá una constante en `descarga.rs` y usala también en la línea 276.
3. `es_fuente_de_secretos` queda como la única definición.

**Test en rojo primero:**
- Unitario: un hallazgo de la descarga con un secreto, marcado como previo, bloquea. Hoy no lo reconoce como secreto.

### 2. La regla de `ENV` o `ARG`

1. La regla "secreto literal en ENV o ARG" de `reglas_dockerfile` emite con la fuente de secretos de configs, `SONDA_SECRETOS`.
2. Las demás reglas de Dockerfile siguen con la fuente `dockerfile`.

**Tests en rojo primero:**
- Unitario: un `Dockerfile` con un secreto en `ENV`, igual en `bootstrap/` y en el repo, da "bloqueado", con la nota "estaba en el estado inicial; los secretos bloquean siempre".
- Guarda: un `Dockerfile` que corre como root, igual en `bootstrap/` y en el repo, no bloquea y sale en "Avisos previos". Es el test f406124 de la 104b.

### 3. Un test por emisor de secretos

1. Un test por cada sonda que da secretos: gitleaks, `configs-secretos`, la regla de `ENV` o `ARG` y la descarga.
2. Cada test arma un hallazgo previo de esa sonda, y comprueba que bloquea.
3. Un test más recorre `FUENTES_DE_SECRETOS` y comprueba que cada fuente de la lista tiene su test. Así una fuente nueva que se olvide hace fallar la suite.

Este punto es de guardas: los tests pasan desde el principio, salvo el que cubre el punto 1.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

**Después de mezclar, lo hace el orquestador, no vos:** la medición de cuatro variantes sobre una copia de `ROOT_HOME` con t1, descrita en la vuelta del 2026-11-14 (27) del plan.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Secretos de prueba.** Usá solo valores falsos con forma de clave, como `CLAVE_AWS` de los tests de hoy. Nunca una clave real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-105.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Emisores de secretos**: la lista final de fuentes, con su constante y su test.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
