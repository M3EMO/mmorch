# Brief de la Tarea 110 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `claude/root-e7813e` en 64361fa. Ahí ya están mezcladas las Tareas 84 a 109.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-110`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, marca de "no aplica" | `src/tipos.rs`, `src/oracle/mod.rs`, las sondas que hoy dan `sin_veredicto` porque la tarea no pide la dimensión, y `docs/interfaces.md` |
| 2, la regla de P103 a | `src/oracle/informe.rs` |

- El punto 2 usa la marca del punto 1. Van en serie.
- **No toques:** `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P103, literal:** "a". La opción decía: "'Listo' exige veredicto en toda dimensión que aplica. Sin juez, la decisión es 'sin decisión'."

**Lo que pasa hoy.** Una evaluación de t1 sin clave de API da "listo", aunque Diseño y Arquitectura quedaron sin veredicto. `sin_decision` (`src/oracle/informe.rs:327`) solo dispara por dos cosas:
- una dimensión sin veredicto cuya rúbrica tiene ítems que bloquean, como Seguridad;
- Completitud sin veredicto.

**El obstáculo.** Hoy `Puntaje.sin_veredicto` mezcla dos casos distintos:
- **No aplica:** la tarea no pide la dimensión. Por ejemplo, Eficiencia sin `[eficiencia]`, Entrega sin `[entrega]`, o Arquitectura sin módulos ni capas (`MOTIVO_SIN_MODULOS`, `src/oracle/perfiles/rust.rs`).
- **No se pudo medir:** el juez no corrió, una herramienta falta, o la infraestructura falló.

Con la regla nueva, solo el segundo caso da "sin decisión".

## Tarea

La regla que manda: **"listo" sale solo si cada dimensión que aplica tiene veredicto.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Marca de "no aplica"

1. `Puntaje` suma `no_aplica: Option<String>`, con `#[serde(default)]`. Lleva el motivo cuando la tarea no pide la dimensión.
2. **Buscá cada lugar que hoy da `sin_veredicto` porque la dimensión no aplica.** Uní codegraph y grep sobre los constructores de `Puntaje` y de `ResultadoSonda` con motivo. En cada uno, decidí si es "no aplica" o "no se pudo medir". El informe lista cada lugar y su decisión.
3. Un puntaje que no aplica lleva `no_aplica` lleno y `sin_veredicto` vacío.
4. Un puntaje que no se pudo medir sigue con `sin_veredicto`, como hoy.
5. Un veredicto viejo sin el campo se lee con `no_aplica` vacío.
6. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: una tarea sin `[eficiencia]` da Eficiencia con `no_aplica` lleno y `sin_veredicto` vacío.
- Unitario: una tarea sin `[entrega]` da Entrega con `no_aplica` lleno.
- Unitario: Arquitectura sin módulos ni capas da `no_aplica`, con el motivo "sin módulos".
- Unitario: Diseño sin juez da `sin_veredicto`, no `no_aplica`.

### 2. La regla de P103 a

1. `sin_decision` dispara por toda dimensión con `sin_veredicto`, sea cual sea. Una dimensión con `no_aplica` no dispara.
2. La regla se llama `dimension_sin_veredicto: <clave>`. Su frase nombra la dimensión y su motivo. Por ejemplo: "Diseño no tiene veredicto: el juez no corrió".
3. Las dos reglas de hoy siguen, con su texto: la de una dimensión que bloquea, y la de Completitud.
4. El orden de `decidir` no cambia: bloqueos, "no listo", "sin decisión" y "listo".
5. **Dimensión "inestable en esta tarea".** Leída al pie de la letra, la regla del usuario la cuenta como sin veredicto. Implementá esa lectura. El usuario puede relajarla en P104. Dejá esa regla en un solo lugar, con un comentario que nombre P104.
6. **El resumen** suma una línea: "Sin medir: <dimensiones>". Con una sola, va en singular.

**Tests en rojo primero:**
- Unitario: un veredicto con todo en verde y Diseño sin veredicto por falta de juez da "sin decisión", con la regla `dimension_sin_veredicto: diseno`.
- Unitario: el mismo veredicto con Eficiencia en `no_aplica` y Diseño con veredicto da "listo".
- Unitario: un veredicto con un test oculto en rojo y sin juez da "no listo", no "sin decisión".
- Unitario: un veredicto con un secreto y sin juez da "bloqueado".
- Unitario: una dimensión inestable da "sin decisión".
- Unitario: el resumen nombra las dimensiones sin medir.
- Guarda: un veredicto viejo sin `no_aplica` decide como hoy, salvo las dimensiones sin veredicto, que ahora dan "sin decisión". Anotá en el informe qué tests cambiaron de valor por esto.

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

Al terminar, escribí `docs/builds/informe-local-110.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **"No aplica" contra "no se pudo medir"**: cada lugar revisado y su decisión.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
