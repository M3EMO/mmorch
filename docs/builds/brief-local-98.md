# Brief de la Tarea 98 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Base: punta de laptop/tarea-97. El orquestador completa el commit: `________`.
- Las Tareas 84 a 97 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-98`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, operadores de Funciona | `src/oracle/calibra/inyectar.rs` |
| 2, `cuerpo_vaciado` para Completitud | `src/oracle/calibra/inyectar.rs` |

- Los dos puntos tocan `inyectar.rs`. Van en serie: primero el 1.
- **No toques los archivos de la Tarea 97:** `src/oracle/mutacion.rs`, `src/oracle/calibra.rs` y `docs/interfaces.md`. Si registrar el defecto nuevo pide tocar `calibra.rs`, tocalo lo mínimo y explicalo en el informe.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Con la regla "dentro del cambio" de la Tarea 95, casi no hay casos.** El orquestador calibró t1 a t3 de S2 sin API. De 6 defectos, 5 no aplicaron.
- `comportamiento` (`src/oracle/calibra/inyectar.rs`) solo muta `==`. Los cambios de S2 tienen de 2 a 34 líneas y casi no traen `==`.
  - t1 agrega brazos de `match` en `src/pattern/composite_pattern.rs`.
  - t2 cambia `>=` por `>` en `src/content_search/mod.rs:85`.
  - t3 reordena la condición de un `while` en `src/content_search/content_match.rs:38`.
- `modulo_borrado` (`inyectar.rs:568`) solo ve módulos de primer nivel. Los tres cambios caen en módulos anidados.

**El riesgo de una detección segura.** Los tests ocultos se eligieron porque pasan de rojo a verde con el cambio. Por eso:
- Revertir el arreglo da una detección segura. En t2, volver `>` a `>=` recrea el estado inicial.
- Vaciar una función que el cambio agregó recrea el estado inicial.
- Borrar un módulo que se usa rompe el build, y la regla del 101 da rojo siempre.

Una calibración con detecciones seguras no mide nada.

## Tarea

La regla que manda: **una mutación dentro del cambio nunca recrea el estado inicial.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Operadores de Funciona dentro del cambio

1. **Operadores**, sobre líneas de producción dentro de los rangos del cambio, leyendo la línea `limpia`, sin comentarios ni cadenas:
   - comparaciones: `<` con `<=`, `>` con `>=`, `==` con `!=`, en los dos sentidos;
   - lógica: `&&` con `||`, en los dos sentidos;
   - condiciones: quitar o agregar un `!` al principio de la condición de un `if` o de un `while`;
   - literales: `true` con `false`;
   - aritmética: `+ 1` con `- 1`, en los dos sentidos.
2. **Regla anti revertir.** Descartá toda mutación cuya línea resultante, sin espacios al borde, sea igual a alguna línea del mismo archivo en el estado inicial.
3. **Elección.** Entre todos los candidatos que quedan, elegí uno con la semilla, como hoy.
4. **Sin candidatos**, el defecto no aplica, con el motivo de hoy: "ningún lugar del cambio admite este defecto".
5. **Sin `bootstrap/`**, `comportamiento` muta `==` como hoy, con la misma semilla. Así las calibraciones de memora y brasa no cambian.
6. El nombre del defecto sigue siendo `comportamiento_roto`. El caso guarda qué operador se usó.

**Tests en rojo primero:**
- Unitario: un cambio que tiene `a > b` da un candidato `a >= b`.
- Unitario: si el estado inicial tenía `a >= b` en ese archivo, esa mutación se descarta.
- Unitario: un cambio con `if x && y` da `if x || y`, y `if !x && y`.
- Unitario: un operador dentro de una cadena o de un comentario no da candidato.
- Unitario: sin `bootstrap/`, con la misma semilla, la elección es la de hoy.

### 2. `cuerpo_vaciado` para Completitud

1. **Defecto nuevo, `cuerpo_vaciado`**, de la dimensión Completitud. Registralo en la lista de defectos de `inyectar.rs`.
2. Toma una función de producción cuyo cuerpo cruza los rangos del cambio. Reemplaza el cuerpo por `todo!()` y conserva la firma.
3. **Solo si la función ya existía en el estado inicial**, con el mismo nombre y en el mismo archivo. Una función que agregó el cambio no es candidata.
4. Sin candidatos, el defecto no aplica, con su motivo.
5. **`modulo_borrado` queda solo para tareas sin `bootstrap/`.** Con estado inicial, no aplica, con el motivo "con estado inicial, borrar un módulo rompe el build y da una detección segura".
6. Sin `bootstrap/`, `cuerpo_vaciado` no aplica, y Completitud usa `modulo_borrado` como hoy.

**Tests en rojo primero:**
- Unitario: una función que existía en el estado inicial y que el cambio modificó queda con su firma y `todo!()` como cuerpo.
- Unitario: una función nueva del cambio no es candidata.
- Unitario: con `bootstrap/`, `modulo_borrado` no aplica, con su motivo.
- Unitario: sin `bootstrap/`, `modulo_borrado` elige como hoy, con la misma semilla.
- Guarda: la copia con `cuerpo_vaciado` compila en un repo de fixture. Usá un fixture que no pida podman.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Si cambia el esquema, avisalo en el informe: `docs/interfaces.md` es un archivo de la Tarea 97.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-98.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Candidatos en S2, sin API**: cuántos candidatos da cada operador en los cambios de t1, t2 y t3, contados sobre copias de `~/.cache/root-wt/mineria/repos/Canop_broot` en sus commits, dentro de tu árbol de trabajo. Solo leé ese repo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
