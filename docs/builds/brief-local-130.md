# Brief de la Tarea 130 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-129`. Trae las Tareas 84 a 129 y la 127b.
- La 131 se construye a la vez. Toca `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/defectos.rs`, `src/oracle/eficiencia.rs` y `src/oracle/calibra.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-130`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, los envoltorios pasan la carpeta | `src/oracle/memo.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/perfiles/herramientas.rs`, `src/oracle/mutacion.rs`, los archivos de la 131, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La Tarea 127 dio una carpeta fija a la mutación de una calibración.** Con `con_carpeta_de_mutacion`, la mutación llama a `ej.ejecutar_en(cmd, carpeta)`. Así, la base y los casos reusan el `target` y la ruta del crate.

**El reparto 4 del orquestador muestra que la carpeta no siempre llega.** Las cinco corridas de mutación de Pruebas llevan `--in-place`, así que el valor de hilo estaba fijado. Pero:

| Corrida | `/salida` montado | Tiempo |
|---|---|---|
| Base | `sondas-N94ort` | 108 s |
| Caso 1 | `sondas-qDvakO` | 79 s |
| Caso 2 | `sondas-AFf6l5` | 81 s |
| Caso 3 | `evaluar-gX2pzV/mutacion` | 80 s, en frío |
| Caso 4 | `evaluar-gX2pzV/mutacion` | 37,7 s, tibio |

**La causa, en `src/oracle/memo.rs`.**
1. La calibración envuelve el ejecutor de sondas en dos capas de memo. `EjecutorGrabador` (línea 91) graba las salidas de la base. `EjecutorReusador` (línea 147) devuelve en los casos lo que la base grabó.
2. **Ninguno implementa `ejecutar_en`.** Los dos toman el valor por defecto del trait (`src/oracle/perfiles/herramientas.rs:39`). Ese valor llama a `ejecutar` e ignora la carpeta.
3. Su `ejecutar` llama al `ejecutar` del ejecutor real. `EjecutorContenedor::ejecutar` crea un `/salida` nuevo, `sondas-<azar>`, con el `target` vacío.
4. Los casos 3 y 4 no pasaron por el memo. Llegaron directo a `EjecutorContenedor::ejecutar_en`, que sí usa la carpeta.

**El caso 4 mide la ganancia:** 37,7 s con el `target` tibio, contra unos 80 s en frío.

## Tarea

La regla que manda: **un envoltorio de ejecutor pasa la carpeta al ejecutor real.** Hacé un commit. Empieza con su test en rojo.

### 1. Los envoltorios pasan la carpeta

1. **`EjecutorGrabador` implementa `ejecutar_en`.** Llama a `self.real.ejecutar_en(cmd, salida)`, y anota la salida en el memo igual que `ejecutar`.
2. **`EjecutorReusador` implementa `ejecutar_en`.** Si el alcance toma el comando y el memo lo tiene, devuelve la salida grabada, igual que `ejecutar`. Si no, llama a `self.real.ejecutar_en(cmd, salida)`.
3. **La clave del memo** sigue siendo el comando. No cambia con la carpeta.
4. **Un comentario en `memo.rs`** dice por qué los dos envoltorios implementan `ejecutar_en`: el valor por defecto del trait ignora la carpeta. El trait vive en `herramientas.rs`, fuera de la tabla, y no se toca.
5. **Buscá otros envoltorios de producción.** Uní codegraph y `grep -n "impl EjecutorHerramienta for"` en `src/`. Fuera de los tests, deberían quedar solo `EjecutorContenedor` y los dos de `memo.rs`. Si aparece otro, anotalo en el informe y no lo cambies.

**Tests en rojo primero, con un ejecutor falso que anota la carpeta de cada llamada:**
- Unitario: `EjecutorGrabador::ejecutar_en` llega al real con la misma carpeta. Hoy llega por `ejecutar`, sin carpeta.
- Unitario: `EjecutorGrabador::ejecutar_en` anota la salida en el memo.
- Unitario: `EjecutorReusador::ejecutar_en` con un comando grabado devuelve la salida grabada, sin llamar al real.
- Unitario: `EjecutorReusador::ejecutar_en` con un comando no grabado llega al real con la misma carpeta.
- Guarda: los tests de `memo.rs` de hoy siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La medición de la ganancia:** el orquestador repite el reparto del tiempo de la vuelta (42), sección 6, con el binario de la 130. Todas las corridas de mutación de una calibración deben montar la misma carpeta `.../mutacion`. La base sigue en frío. Los casos deben bajar a unos 38 s.

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

Al terminar, escribí `docs/builds/informe-local-130.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los envoltorios de producción**: cada `impl EjecutorHerramienta for` fuera de los tests, y si pasa `ejecutar_en`.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
