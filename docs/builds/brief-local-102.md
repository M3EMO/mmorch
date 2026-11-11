# Brief de la Tarea 102 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-101` (cabb373). Las Tareas 84 a 101 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-102`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Grupo | Podés tocar |
|---|---|
| A, lector de Rust e inyector | `src/oracle/perfiles/rust.rs`, `src/oracle/calibra/inyectar.rs`, y la constante de versión de sondas en `src/oracle/calibra.rs` |
| B, `no_compila` y aviso | `src/oracle/mod.rs` |
| C, aviso de `con_tests/` | `src/oracle/calibra.rs` |

- Los grupos B y C no comparten archivos con A, salvo la constante de versión de sondas. Si A y C tocan `calibra.rs` a la vez, A cambia solo esa constante, y C mezcla después.
- Dentro del grupo A, primero `analizar` y después `inyectar.rs`.
- `docs/interfaces.md` se regenera una sola vez, al final, si cambia.
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

**C3 de la Tarea 101 quedó sin hacer.** `analizar` (`src/oracle/perfiles/rust.rs:292`) limpia cada línea por separado, con `limpiar_linea`. No recuerda si la línea anterior abrió un comentario de bloque o una cadena cruda. Además, su línea `limpia` no conserva las posiciones en bytes.
- El inyector no puede usarlo, y sigue con su propio `linea_limpia`, que tampoco ve esos casos. Una línea dentro de `/* */` se puede mutar, y el caso no mide nada.
- Las sondas tampoco los ven. Un `unsafe` o un `use crate::x` dentro de un comentario de bloque cuenta hoy como código.
- El test en rojo que dejó la 101 está en `/home/mpera/.cache/root-wt/logs-local101c/inyectar-c3-rojo-sin-commit.rs`.

**Un build roto solo por la suite oculta no marca `no_compila`** (`src/oracle/mod.rs:761`). La regla A1 de la 101 no ve ese caso en la calibración. Una mutación que rompe solo los tests ocultos puede contar como detección por construcción.

**Tres detalles del informe de la 101:**
- Con `--repo` y `con_tests/` a la vez, gana `con_tests/` sin aviso.
- El aviso dice "corrió 1 mutantes".
- `bucle` en `inyectar.rs` tiene su propia detección de funciones, y excluye `const fn`, `async fn` y `unsafe fn`.

## Tarea

La regla que manda: **un solo lector de Rust para sondas e inyector, y ningún build roto se cuenta como detección.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### Grupo A: lector de Rust e inyector

**A1. `analizar` con estado entre líneas.**
1. `analizar` recuerda entre líneas si está dentro de un comentario de bloque `/* ... */`, que puede anidarse, o dentro de una cadena cruda `r"..."` o `r#"..."#`, o de una cadena común que sigue en la línea siguiente.
2. En `limpia`, cada carácter de comentario o de cadena se reemplaza por espacios. La cantidad de bytes es la misma que la del original. Así una posición en bytes de `limpia` vale en la línea cruda.
3. Los delimitadores de una cadena pueden quedar como `"` para que las sondas sigan viendo que hay una cadena. Mirá qué espera cada sonda hoy, y anotalo en el informe.
4. **Versión de sondas.** Las dimensiones cuyas sondas usan `analizar` suben su versión (Tarea 89). Anotá cuáles en el informe. Sus filas viejas dejan de valer, y se recalibran sin API.

**A2. El inyector usa `analizar`.**
1. `inyectar.rs` lee las líneas con `analizar`, en vez de `linea_limpia`.
2. Si `linea_limpia` queda sin uso, borrala después de unir los llamadores.

**A3. `bucle` con calificadores.** La detección de funciones de `bucle` usa los mismos calificadores que `nombre_de_funcion`.

**Tests en rojo primero:**
- El test que dejó la 101 en `logs-local101c/inyectar-c3-rojo-sin-commit.rs`.
- Unitario: una línea dentro de un comentario de bloque de tres líneas sale toda en espacios en `limpia`, con el mismo largo en bytes.
- Unitario: una línea dentro de `r#"..."#` de varias líneas sale en espacios en `limpia`.
- Unitario: un `/* /* */ */` anidado se cierra en el segundo `*/`.
- Unitario: un `unsafe` dentro de un comentario de bloque no da hallazgo de Seguridad.
- Unitario: un `use crate::x` dentro de un comentario de bloque no da arista de Arquitectura.
- Unitario: `bucle` reconoce `pub(crate) const fn`.
- Guarda: las sondas dan los mismos hallazgos que hoy en sus fixtures sin comentarios de bloque.

### Grupo B: `src/oracle/mod.rs`

**B1. `no_compila` también con la suite oculta.**
1. Con `runner = "nextest"`, si la suite oculta sale con 101, el veredicto marca `no_compila`, aunque el contrato haya compilado.
2. La regla de la Tarea 95 sigue: cada test oculto esperado cuenta como fallo.

**B2. El singular del aviso.** Con N igual a 1, el aviso dice "cargo-mutants corrió 1 mutante que no estaba en la muestra".

**Tests en rojo primero:**
- Unitario: contrato en 0 y oculto en 101 dan `no_compila: true`, y los ocultos en fallo.
- Unitario: el aviso con N igual a 1 va en singular.
- Guarda: contrato en 0 y oculto en 100 no marcan `no_compila`.

### Grupo C: `src/oracle/calibra.rs`

**C1. Aviso de `con_tests/`.** Si `calibrar --dimension pruebas` recibe `--repo` y la tarea trae `con_tests/`, usa `con_tests/` y avisa: "se usó con_tests/ en vez de --repo".

**Test en rojo primero:**
- Unitario: con `--repo` y `con_tests/`, la salida trae el aviso, y la fila guarda `repo_origen` en `"con_tests"`.

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

Al terminar, escribí `docs/builds/informe-local-102.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con grupo, punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Sondas que cambiaron**: qué sondas usan `analizar`, qué versiones subieron, y si algún hallazgo de los fixtures cambió.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
