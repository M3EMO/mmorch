# Brief de la Tarea 111 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `claude/root-e7813e` en 64361fa. Ahí ya están mezcladas las Tareas 84 a 109.
- **La Tarea 110 se construye a la vez, en `laptop/tarea-110`.** Toca `src/tipos.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs` y varias sondas. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-111`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, calibración por versión | `src/oracle/calibra.rs` |
| 2, la línea de `calibrar` | `src/lib.rs` (solo el texto de `calibrar`), `docs/interfaces.md` |

- El punto 2 usa el resultado del punto 1. Van en serie.
- **No toques:** `src/tipos.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs`, el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P102, literal:** "a". La opción decía: "Para las dimensiones de juez, 'calibrada' pasa a pedir una cota inferior de Wilson de 0,8 o más, con los casos de todas las tareas juntos, por versión de Root. Son unos 16 a 30 casos por dimensión. Para las dimensiones de oráculo, 'calibrada' pide un defecto detectado y todos los limpios bien."

**Lo que pasa hoy.** `Calibraciones::aplicar` (`src/oracle/calibra.rs:229`) marca un puntaje como calibrado con la fila de una sola tarea (`de`, línea 218). Pide `muestra_suficiente` (línea 2776), que son 5 defectos y 5 limpios, y una exactitud balanceada de `UMBRAL`, 0,9 (línea 41). Con 5 casos acertados, la cota inferior de Wilson al 95 % es 0,57. Es una estimación puntual, no una cota.

**Cotas de Wilson al 95 %, con todos los casos acertados:** 5 casos dan 0,57; 16 dan 0,81; 20 dan 0,84; 35 dan 0,90. Con un fallo, 29 de 30 da 0,83.

## Tarea

La regla que manda: **una dimensión se calibra por versión de Root, con los casos de todas las tareas, y solo con una cota que sostenga lo que dice.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Calibración por versión

1. **La versión** es la huella de la fila: prompt del juez, rúbricas y versión de sondas (`huella_de`, Tareas 81 y 89).
2. **Filas que se juntan:** las filas de todas las tareas con la huella actual de la dimensión. Cada fila debe estar vigente: su `tarea_hash` coincide con el `tarea.toml` actual de su tarea, y trae `inyecciones_distintas` (Tarea 106). Para saber el `tarea.toml` actual de otra tarea, leelo desde `ROOT_HOME/oraculos/<tarea>/tarea.toml`. Si no existe, esa fila no cuenta.
3. **Cómo se suman:** los defectos distintos detectados, los defectos distintos medidos, los limpios con veredicto que dieron bien, y los limpios con veredicto.
4. **Dimensiones de juez** (las que tienen ítems `juez` o `mixto`): calibrada si la cota inferior de Wilson al 95 % es de 0,8 o más en la sensibilidad, y también en la especificidad.
5. **Dimensiones de oráculo** (solo sondas): calibrada si hay al menos un defecto detectado, y si todos los limpios con veredicto dieron bien.
6. **Sin casos aplicables en ninguna tarea:** la dimensión queda sin calibrar, con el motivo "ningún caso aplicable en las tareas medidas".
7. **`aplicar`** marca el puntaje de una tarea con el estado de la versión de su dimensión. La regla del 20 % de limpios sin veredicto sigue, ahora sobre los casos juntos.
8. **Una función pura** calcula la cota de Wilson. Usa z = 1,96.
9. **Constantes nuevas:** la cota mínima de 0,8 y el nivel de 95 %, con su motivo en el comentario: la decisión P102 del usuario. `UMBRAL` y `muestra_suficiente` quedan sin uso si nadie más los lee. Borralos después de unir sus llamadores.
10. Una fila vieja, sin `inyecciones_distintas` o sin `tarea_hash`, no cuenta.

**Tests en rojo primero:**
- Unitario: la cota de Wilson de 16 de 16 es 0,806, y la de 5 de 5 es 0,566, con tres decimales.
- Unitario: una dimensión de juez con 5 de 5 en una tarea no calibra. Hoy calibra.
- Unitario: la misma dimensión con 16 de 16 defectos y 16 de 16 limpios, repartidos en 4 tareas con la misma huella, calibra.
- Unitario: una fila con otra huella no se suma.
- Unitario: una fila con un `tarea_hash` que no coincide con el `tarea.toml` actual no se suma.
- Unitario: una dimensión de oráculo con 1 defecto detectado y 3 de 3 limpios bien calibra.
- Unitario: la misma dimensión con un limpio que dio mal no calibra.
- Unitario: una dimensión sin casos aplicables queda sin calibrar, con su motivo.

### 2. La línea de `calibrar`

1. La línea de `calibrar` en `src/lib.rs` muestra, para la dimensión, la cota de Wilson de la versión en sensibilidad y en especificidad, y cuántos casos faltan para llegar a 0,8.
2. En una dimensión de oráculo, muestra si ya hay un defecto detectado, y si todos los limpios dieron bien.
3. Si el esquema de la fila cambia, regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: la línea de una dimensión de juez con 5 de 5 dice la cota 0,57 y que faltan 11 casos sin fallos para llegar a 0,8.
- Unitario: la línea de una dimensión de oráculo dice "1 defecto detectado, limpios bien".

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

Al terminar, escribí `docs/builds/informe-local-111.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Cómo se juntan las filas**: qué huella y qué vigencia se usan, y de dónde se lee el `tarea.toml` de otra tarea.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
