# Brief de la Tarea 124 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-122`. Trae las Tareas 84 a 122.
- La 123 se construye a la vez. Toca `src/oracle/mutacion.rs` y `src/oracle/calibra.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-124`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la línea de una base sin veredicto | `src/lib.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

**La Tarea 121 suma `Fila.base_sin_veredicto`.** Una calibración cuya base queda sin veredicto escribe una fila con ese motivo, y sale bien. La fila no mide nada.

**La salida humana no lo muestra.** `src/lib.rs:204` imprime el motivo solo si la fila trae `inestable` o `no_aplica`. Una fila con `base_sin_veredicto` imprime "sensibilidad sin medir, ..., casos 0". Parece una medición que no encontró casos.

## Tarea

La regla que manda: **una fila que no midió lo dice en la primera línea.** Hacé un commit. Empieza con su test en rojo.

### 1. La línea de una base sin veredicto

1. La salida humana imprime `<tarea>/<dimension>: base sin veredicto: <motivo>` si la fila trae `base_sin_veredicto`.
2. Ese motivo va antes de `inestable` y de `no_aplica`. Una fila con `base_sin_veredicto` no trae los otros dos, pero el orden queda fijo.
3. Si la calibración guardó un archivo de casos, imprime también la línea `casos: <ruta>`, como hoy.
4. La salida con `--json` no cambia.
5. Saca la elección del motivo a una función pura, para probarla sin correr una calibración.

**Tests en rojo primero:**
- Unitario: una fila de `s2-t1/arquitectura` con `base_sin_veredicto` "juicio 1 de la base sin veredicto: cobertura 6 de 10 de peso, menor que dos tercios" da la línea "s2-t1/arquitectura: base sin veredicto: juicio 1 de la base sin veredicto: cobertura 6 de 10 de peso, menor que dos tercios". Hoy da "sensibilidad sin medir".
- Guarda: una fila con `inestable` da su línea de hoy.
- Guarda: una fila medida da su línea de hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

Ninguno. La tarea solo cambia texto de la salida.

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

Al terminar, escribí `docs/builds/informe-local-124.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
