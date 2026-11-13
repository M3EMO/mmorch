# Brief de la Tarea 106 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-105` (3a1ed58). Las Tareas 84 a 105 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-106`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, elección sin reposición | `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra.rs` |
| 2, `inyecciones_distintas` | `src/oracle/calibra.rs`, `src/lib.rs` (solo la línea de texto de `calibrar`), `docs/interfaces.md` |
| 3, equivalentes curados | `src/oracle/tarea.rs`, `src/oracle/calibra/inyectar.rs` |
| 4, duración por mutante | `src/oracle/mutacion.rs` |

- Los puntos 1, 2 y 3 comparten archivos. Van en serie: 1, 2 y 3.
- El punto 4 corre en paralelo.
- `docs/interfaces.md` se regenera una sola vez, al final.
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

**La calibración repite la misma inyección dentro de una corrida.** El orquestador lo midió en `/home/mpera/.cache/root-wt/orquestador/medicion-107.md`. En 10 de 15 corridas con 2 o más defectos, una inyección sale 2 o 3 veces. En t1 Funciona, con la semilla 43, los 3 defectos son la misma: `composite_pattern.rs:113`, `true` a `false`.

**La causa** (`src/oracle/calibra.rs`, cerca de la línea 928):
- Cada caso con defecto toma su propia semilla: `semilla + i`.
- El inyector arma los candidatos del defecto y elige uno con esa semilla, con `rng.elegir`, en cada función de `src/oracle/calibra/inyectar.rs`.
- Ningún caso sabe qué eligieron los otros. Los casos corren en hilos.
- Con uno o dos candidatos dentro del cambio, repetir es lo esperable.

Los totales cuentan casos, no inyecciones distintas. Una cota de Wilson sobre esos números miente.

**Un mutante equivalente conocido.** `composite_pattern.rs:113`, `(And, None) => true` a `false`, está en la función de atajo de `eval`. Con `false`, el operador recibe `(And, None, _)` y también da `None`. Ningún test lo puede matar. Root no puede probarlo solo, pero una persona sí, leyendo el código.

**Falta el tiempo de cada mutante.** Para decidir si los 13 "delete field" pesan, hace falta su duración. Hoy solo se lee la duración de la base (`src/oracle/mutacion.rs:251`).

## Tarea

La regla que manda: **cada caso de una calibración mide una inyección distinta, y toda curación humana queda a la vista.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Elección sin reposición

1. **Identidad de una inyección:** el defecto, el archivo, la línea y la edición, que es el operador de la Tarea 98 o el tipo de cuerpo vaciado.
2. **`Contexto` suma `excluidos`:** un conjunto de identidades. Cada función del inyector quita esos candidatos antes de elegir.
3. **Elección en serie.** Antes de evaluar, `calibra.rs` elige la inyección de cada caso con defecto, en orden, y suma cada una a `excluidos`. La evaluación sigue en paralelo.
4. **Reemplazos.** Un reemplazo elige con `excluidos`, que guarda las usadas y las inválidas. Durante la evaluación en hilos, `excluidos` vive detrás de un cerrojo.
5. **Candidatos agotados.** Si no quedan candidatos distintos, el caso queda "no aplica: candidatos distintos agotados". Nunca se repite una inyección.
6. Con la misma semilla, la elección es determinista.

**Tests en rojo primero:**
- Unitario: con un solo candidato y n = 6, hay un caso con defecto medido y los demás quedan "candidatos distintos agotados". Hoy hay tres casos iguales.
- Unitario: con tres candidatos y tres casos con defecto, los tres son distintos.
- Unitario: un reemplazo no repite una inyección usada ni una inválida.
- Unitario: dos corridas con la misma semilla dan las mismas inyecciones.

### 2. `inyecciones_distintas`

1. `Fila` suma `inyecciones_distintas: Option<u32>`, con `#[serde(default)]`.
2. La sensibilidad se calcula sobre inyecciones distintas. Con el punto 1, cada caso con defecto ya es distinto.
3. "Muestra suficiente" cuenta defectos distintos. El mínimo no cambia.
4. **Filas viejas.** Una fila sin el campo se lee como hoy. La línea de `calibrar` la marca: "fila anterior a la Tarea 106: puede contar inyecciones repetidas".
5. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: una fila nueva trae `inyecciones_distintas`.
- Unitario: la línea de `calibrar` de una fila sin el campo trae la marca.
- Guarda: una fila vieja se lee y calibra como hoy.

### 3. Equivalentes curados

1. `Tarea` suma una sección opcional `[calibracion]`, con `equivalentes`: una lista de entradas con `archivo`, `linea`, `edicion` y `motivo`. Todo con `#[serde(default)]`.
2. El inyector excluye esas identidades, igual que `excluidos`.
3. Una entrada sin `motivo` es un error de carga del `tarea.toml`, con el mensaje "un equivalente necesita su motivo".
4. La fila suma `equivalentes_excluidos: u32`, con `#[serde(default)]`.
5. Root nunca escribe esa lista. La escribe una persona.

**Tests en rojo primero:**
- Unitario: un equivalente declarado en `tarea.toml` no se elige nunca.
- Unitario: una entrada sin `motivo` da el error de carga.
- Unitario: la fila cuenta cuántos equivalentes excluyó.
- Guarda: un `tarea.toml` sin `[calibracion]` carga como hoy.

### 4. Duración por mutante

1. El estado por mutante de la Tarea 97 suma la duración de cada mutante, en segundos, leída del `outcomes.json`.
2. Si el reporte no trae la duración, el campo queda vacío.
3. El campo lleva `#[serde(default)]`.

**Tests en rojo primero:**
- Unitario: un `outcomes.json` con duraciones da la duración de cada mutante en su estado.
- Guarda: un reporte sin duraciones da el estado de hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

**Después de mezclar, lo hace el orquestador, no vos:** agregar a `oraculos/s2-t1/tarea.toml` el equivalente `composite_pattern.rs:113`, `true` a `false`, con su motivo.

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

Al terminar, escribí `docs/builds/informe-local-106.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Identidad de una inyección**: cómo se arma en cada defecto.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
