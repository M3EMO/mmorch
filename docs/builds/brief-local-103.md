# Brief de la Tarea 103 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Base: punta de laptop/tarea-102. El orquestador completa el commit: `________`.
- Las Tareas 84 a 102 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-103`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Grupo | Podés tocar |
|---|---|
| A, línea base y marca de previo | `src/oracle/previos.rs` (nuevo), `src/oracle/mod.rs`, `src/tipos.rs`, `docs/interfaces.md` |
| B, bloqueo, puntaje e informe | `src/oracle/informe.rs` |

- El grupo B usa el campo que agrega el A. Va después.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P96, literal:** "a". Es la opción "Bloquea solo lo que el cambio introduce. Lo previo se informa, y no bloquea. Un secreto bloquea siempre."

**El problema.** Toda tarea de broot da "bloqueado". `bloqueos` (`src/oracle/informe.rs:369`) bloquea por todo hallazgo de Seguridad con severidad 3. El `Cargo.lock` de broot ya traía 7 avisos RUSTSEC antes del cambio. Ninguna evaluación real de S2 puede dar otra decisión.

**Lo que ya existe y sirve:**
- Cada hallazgo trae un `id` armado por `crear_hallazgo` (`src/oracle/perfiles/mod.rs:64`) con la dimensión, el archivo, la cita y el detalle. La línea no entra al id.
- En cargo audit (`src/oracle/perfiles/herramientas.rs`, cerca de la línea 248), la cita es `<crate> <versión>` y el detalle empieza con el id del aviso. Así el id ya junta el aviso, el crate y la versión.
- `cambios::rangos_cambiados` da las líneas que cambió el commit, comparando con `bootstrap/` (Tarea 81).

## Tarea

La regla que manda: **solo bloquea lo que el cambio introduce, y la línea base sale del oráculo, nunca del repo evaluado.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### Grupo A: línea base y marca de previo

**A1. Línea base de Seguridad.**
1. Módulo nuevo, `src/oracle/previos.rs`. Si la tarea trae `oraculos/<tarea>/bootstrap/`, corre las sondas de Seguridad sobre ese estado: cargo audit, gitleaks y las sondas nativas. Usa los mismos comandos y la misma imagen que la evaluación.
2. El resultado es la lista de ids de la línea base, con cuántas veces aparece cada uno, y su línea.
3. **Memoria.** La línea base se guarda en `ROOT_HOME`, con una clave que junta el hash de `bootstrap/`, la versión de las sondas de Seguridad y la imagen. Una segunda evaluación de la misma tarea no la recalcula. Si hay un memo en Root que ya sirve para esto, usalo.
4. Sin `bootstrap/`, no hay línea base.

**A2. La marca de previo.**
1. `Hallazgo` suma `previo: bool`, con `#[serde(default)]`.
2. Solo en la dimensión Seguridad, después de correr las sondas sobre el repo evaluado:
   - **cargo audit:** previo si su id está en la línea base.
   - **sondas nativas:** previo si su id está en la línea base y su línea queda fuera de los rangos cambiados.
   - **gitleaks:** previo si su id está en la línea base. Solo informa: un secreto bloquea igual.
3. **Ids repetidos.** Si el repo trae un id más veces que la línea base, las de más son nuevas. Las que caen dentro del cambio son nuevas primero.
4. Sin `bootstrap/`, ningún hallazgo es previo. El veredicto suma el aviso "sin estado inicial: todo hallazgo de Seguridad cuenta como nuevo".
5. La línea base sale solo de `oraculos/<tarea>/bootstrap/`. Nunca leas un estado inicial desde el repo evaluado.
6. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un aviso de cargo audit con el mismo aviso, crate y versión que la línea base queda previo.
- Unitario: el mismo aviso con otra versión del crate queda nuevo.
- Unitario: un crate vulnerable que no estaba en la línea base queda nuevo.
- Unitario: un hallazgo nativo con el id de la línea base, pero en una línea del cambio, queda nuevo.
- Unitario: con un id dos veces en el repo y una vez en la línea base, uno queda nuevo, y es el que cae dentro del cambio.
- Unitario: sin `bootstrap/`, nada queda previo, y está el aviso.
- Unitario: la segunda evaluación de la misma tarea usa la línea base guardada y no corre las sondas sobre `bootstrap/`.

### Grupo B: bloqueo, puntaje e informe

**B1. Bloqueo.** `bloqueos` solo dispara por un hallazgo de Seguridad con severidad 3 que no es previo, o por cualquier secreto de gitleaks, sea previo o no.

**B2. Puntaje de Seguridad.** El puntaje cuenta solo los hallazgos nuevos, más los secretos previos. Si el puntaje se calcula fuera de `informe.rs`, por ejemplo en `mod.rs`, hacelo en el grupo A y explicalo en el informe.

**B3. Informe del PM.**
1. Una sección "Avisos previos" lista cada hallazgo previo que no es secreto: fuente, aviso o regla, crate y versión o archivo, y severidad. Termina con "estaban en el estado inicial; no bloquean".
2. El resumen suma la línea "N avisos previos de Seguridad, no bloquean". Con N igual a 1, va en singular.
3. Un secreto previo va con los bloqueos, con la nota "estaba en el estado inicial; los secretos bloquean siempre".
4. Sin `bootstrap/`, la sección no aparece.

**Tests en rojo primero:**
- Unitario: un veredicto con 7 avisos de cargo audit previos de severidad 3 y nada nuevo no da "bloqueado".
- Unitario: el mismo veredicto con un aviso nuevo de severidad 3 da "bloqueado", por ese aviso.
- Unitario: un secreto previo da "bloqueado", con su nota.
- Unitario: el informe trae la sección "Avisos previos" y la línea del resumen.
- Guarda: un veredicto viejo sin el campo `previo` se lee con `false`, y su decisión no cambia.

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

Al terminar, escribí `docs/builds/informe-local-103.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con grupo, punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Dónde vive la línea base**: la ruta en `ROOT_HOME`, su clave y cuándo se recalcula.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
