# Brief de la Tarea 132 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-131`. Trae las Tareas 84 a 131 y la 127b.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-132`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, cuándo se calibra Eficiencia | `src/oracle/calibra.rs` |
| 2, los defectos que la tarea puede recibir | `src/oracle/calibra.rs` |

- Los dos puntos tocan el mismo archivo. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/eficiencia.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/perfiles/`, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La calibración de Eficiencia de s2-t1 no midió nada.** Con el binario de la cadena 110 a 131, dio n=0 y "no aplica: la tarea no define [eficiencia]". La Tarea 131 ya sumó el defecto `LastreDeMemoria` y la referencia reusada.

**La causa, en `src/oracle/calibra.rs:2207`.** Antes de la base, `calibrar_tarea_con_lector` corta si la dimensión es Eficiencia y la tarea no trae `[eficiencia]`. Escribe una fila `no_aplica` con `MOTIVO_NO_APLICA_EFICIENCIA` (línea 2356) y sale. Su comentario dice: "Es el mismo criterio que la sonda usa para no medir."

**Ese criterio quedó viejo con la Tarea 115.** Desde la 115, la evaluación mide Eficiencia sin `[eficiencia]` si la tarea es de Rust y trae `bootstrap/`. Lo decide `puntaje_de_eficiencia` (`src/oracle/mod.rs:1931`):
- Sin `[eficiencia]` y fuera de Rust: `MOTIVO_SIN_EFICIENCIA`.
- Sin `[eficiencia]` y sin `bootstrap/`: `MOTIVO_SIN_ESTADO_INICIAL`.
- `es_no_aplica` (`src/oracle/mod.rs:2122`) convierte los dos en `no_aplica`.
- En los demás casos, la evaluación mide.

**Un segundo hueco.** `de_dimension(Eficiencia)` trae `BucleCuadratico` y `LastreDeMemoria`. `BucleCuadratico` necesita el camino caliente de `[eficiencia]`. Sin esa sección, cada caso que lo elige queda no aplicable. Con la rotación de la Tarea 128, la mitad de los casos con defecto se perderían.

## Tarea

La regla que manda: **la calibración mide Eficiencia cuando la evaluación la mide, y solo rota defectos que la tarea puede recibir.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Cuándo se calibra Eficiencia

1. **El corte de la línea 2207** vale solo si la evaluación tampoco mediría Eficiencia: sin `[eficiencia]`, y además fuera de Rust o sin `bootstrap/` en la carpeta de la tarea.
2. **El motivo de la fila** es el mismo que daría la evaluación. Fuera de Rust, el de `MOTIVO_SIN_EFICIENCIA`. Sin `bootstrap/`, el de `MOTIVO_SIN_ESTADO_INICIAL`. Usá las constantes de `perfiles::herramientas`, sin copiar el texto.
3. **Si las constantes no se alcanzan desde `calibra.rs`,** detenete y escribilo en el informe. No las copies.
4. **Con Rust y `bootstrap/`,** la calibración sigue: corre la base y los casos, como cualquier dimensión.
5. Si `MOTIVO_NO_APLICA_EFICIENCIA` queda sin uso, borralo después de unir sus llamadores.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: una tarea de Rust con `bootstrap/` y sin `[eficiencia]` corre la base y los casos de Eficiencia. Hoy escribe la fila `no_aplica` y sale.
- Unitario: una tarea de Python sin `[eficiencia]` da `no_aplica` con el motivo de `MOTIVO_SIN_EFICIENCIA`.
- Unitario: una tarea de Rust sin `bootstrap/` ni `[eficiencia]` da `no_aplica` con el motivo de `MOTIVO_SIN_ESTADO_INICIAL`.
- Guarda: una tarea con `[eficiencia]` se calibra como hoy.

### 2. Los defectos que la tarea puede recibir

1. **Antes de rotar,** la calibración saca de la lista de `de_dimension` los defectos que la tarea no puede recibir.
2. **Hoy hay un caso:** `BucleCuadratico` sin `[eficiencia]`.
3. **La regla vive en una sola función** de `calibra.rs`, con un comentario que nombre el motivo. Se aplica en los dos lugares que llaman a `de_dimension` (cerca de las líneas 1553 y 2760).
4. **Si la lista queda vacía,** la calibración escribe la fila `no_aplica` con el motivo "ningún defecto aplica a la tarea", sin correr la base.
5. **La rotación de la 128** sigue, sobre la lista filtrada.

**Tests en rojo primero:**
- Unitario: sin `[eficiencia]`, todos los casos con defecto de Eficiencia usan `LastreDeMemoria`. Hoy la mitad usa `BucleCuadratico`.
- Unitario: con `[eficiencia]`, la rotación usa los dos, como la 128.
- Unitario: una lista vacía da la fila `no_aplica` con su motivo, sin llamar al evaluador.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de s2-t1, s2-t2 y s2-t3,** con n=4 y sin API, con el binario de la cadena con la 132. Se corre en el `ROOT_HOME` real, con un respaldo de `calibracion.json` y `calibracion/` antes, como la recalibración de oráculo de la vuelta (47).
- Que `LastreDeMemoria` suba `memory.peak` en los casos, y que los limpios queden dentro del ruido.

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

Al terminar, escribí `docs/builds/informe-local-132.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La condición de Eficiencia**: el texto exacto de la condición nueva, y de dónde sale cada motivo.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada símbolo borrado o firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
