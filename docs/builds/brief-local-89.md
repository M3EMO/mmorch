# Brief de la Tarea 89 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-88` (76f95c6). Las Tareas 84 a 88 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-89`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, imports agrupados | `src/oracle/perfiles/rust.rs`, `src/oracle/capas.rs` |
| 2, huella de la fila | `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 3, mensajes del `rm` | `src/corre/ejecutor.rs` |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** Antes de buscar llamadores, corré `codegraph init .` y `codegraph sync` en tu worktree. En la Tarea 88 no devolvió llamadores por falta de índice. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**(h) La sonda de Arquitectura no ve los imports agrupados.** `usos_de_crate` (`src/oracle/perfiles/rust.rs:591`) busca `crate::nombre` dentro de una sola línea. Muchos repos de Rust importan así:

```rust
use {
    crate::{
        app::*,
        display::Screen,
    },
    std::io,
};
```

La línea `crate::{` da un nombre vacío, y las líneas siguientes no se leen. En broot, `r00t oraculo capas` puso a `cli` en la capa de abajo, como si no usara a ningún módulo. La sonda pierde la mayoría de las dependencias. Entonces no ve los ciclos ni los usos hacia una capa anterior que esos imports esconden. Las capas que propone `r00t oraculo capas` salen del mismo grafo, así que también están mal.

**Huella de la fila.** La huella de `Fila` junta el prompt del juez y las rúbricas. No junta el `tarea.toml` ni el código de las sondas. Cambiar `capas` o arreglar una sonda no invalida las filas viejas.

**Mensajes.** El informe de la Tarea 88 dejó dos mensajes confusos. El `rm` que corre después de una señal dice "no se borró" aunque la señal ya borró el contenedor. El fallo de `correr_paso_de_quitar` no nombra el contenedor. Además, `FALTA_CAPA` en `capas.rs` repite el texto de `capas_sin_modulo`.

## Tarea

La regla que manda: **la sonda ve cada dependencia que el compilador ve, y una calibración vale solo para las condiciones que la midieron.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Imports agrupados

1. **Sentencias `use` completas.** En `aristas_de`, una sentencia `use` se lee entera, desde `use` hasta su `;`, aunque ocupe varias líneas. Vale también con `pub use`, `pub(crate) use` y otras visibilidades.
2. **Árbol del `use`.** Dentro de la sentencia, cada camino que empieza con `crate::` aporta su primer segmento como destino. Eso incluye los grupos: `crate::{a::X, b::{c, d}}` da `a` y `b`.
3. **Grupo exterior.** `use { crate::{a, b}, std::io };` da `a` y `b`.
4. **Uso fuera de `use`.** Un `crate::x::f()` en una expresión sigue contando, como hoy.
5. **Línea y cita.** El `Uso` de cada arista guarda la línea donde aparece su nombre, y esa línea como cita.
6. **Tests del repo.** Una sentencia `use` dentro de un bloque de test no cuenta, como hoy.
7. **Comentarios y cadenas** no cuentan. Usá `limpia`, como hoy.
8. **`FALTA_CAPA` único.** Una sola constante en `rust.rs`, que usan `capas_sin_modulo` y `capas.rs`.

**Tests en rojo primero:**
- Unitario: el bloque de ejemplo de arriba da aristas hacia `app` y `display`.
- Unitario: `use crate::{a::X, b::{c, d}};` en una línea da `a` y `b`.
- Unitario: `pub(crate) use crate::{\n    a,\n};` da `a`.
- Unitario: un ciclo escondido en imports agrupados da el hallazgo de ciclo.
- Unitario: un uso hacia una capa anterior escondido en un import agrupado da su hallazgo.
- Unitario: `r00t oraculo capas` sobre un fixture con imports agrupados propone las capas correctas.
- Guarda: un `use` agrupado dentro de `#[cfg(test)]` no da arista.
- Guarda: los tests de la sonda de Arquitectura y de `capas` siguen verdes.

**Medición para el informe, sin API.** Corré `r00t oraculo capas --repo <dir> --json` antes y después del arreglo, sobre `~/.cache/root-wt/mineria/repos/Canop_broot` en el commit `1f77962e6ff3`, con `git worktree` o `git archive` dentro de tu árbol de trabajo. Informá cuántas aristas, ciclos y capas da cada uno. Solo leé ese repo: no lo modifiques.

### 2. Huella de la fila

1. **`tarea_hash`.** `Fila` suma `tarea_hash: Option<String>`, con `#[serde(default)]`. `calibrar` lo llena con el hash del `tarea.toml` de la tarea.
2. **Vigencia.** En `aplicar`, una fila con `tarea_hash` distinto del actual no calibra. Una fila sin `tarea_hash` sigue valiendo.
3. **Versión de sondas.** Cada dimensión tiene una constante con la versión de sus sondas. `huella_de` la suma, también en las dimensiones solo de sondas.
4. **Arquitectura sube su versión,** por el punto 1.
5. **Motivo.** Una fila que no vale por `tarea_hash` da el motivo "calibración con otro tarea.toml". Una que no vale por la versión de sondas da "calibración de otro juez", como hoy.

**Tests en rojo primero:**
- Unitario: una fila con `tarea_hash` distinto no calibra, y da su motivo.
- Unitario: una fila sin `tarea_hash` sigue calibrando.
- Unitario: subir la versión de sondas de Arquitectura cambia la huella de Arquitectura, y no la de Seguridad.
- Guarda: una fila de una dimensión solo de sondas, sin huella, sigue calibrando, como hoy.

### 3. Mensajes del `rm`

1. Si la señal ya terminó el hijo, el `rm` del camino del error no corre. La limpieza de la señal ya quitó los contenedores.
2. El fallo de `correr_paso_de_quitar` nombra el contenedor, como `cortar_vencido` y `error_con_borrado`.

**Tests en rojo primero:**
- Unitario: un hijo terminado por la señal no dispara un segundo `rm`.
- Unitario: el error de `correr_paso_de_quitar` trae el nombre del contenedor.

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

Al terminar, escribí `docs/builds/informe-local-89.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Medición sobre broot**: aristas, ciclos y capas, antes y después del punto 1.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
