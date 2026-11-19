# Brief de la Tarea 166 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (86).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-165` (372af94). Trae las Tareas 84 a 165 y la 162b.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-166`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las huellas de un diff y los `use` de Rust | `src/observe/transcript.rs` |
| 2, el comando `corre revisar-lectura` | `src/observe/transcript.rs`, `src/cli.rs`, `src/lib.rs` |

- Los dos puntos comparten `transcript.rs`. Van en serie: primero el 1.
- **`src/lib.rs` está en la lista a propósito.** Cablea las acciones de la línea de comandos.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/corre/`, `src/oracle/`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila`, `Veredicto` ni `Evento`.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real ni a ningún builder real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`. El orquestador lo corre al revisar.

## Por qué

**Root marcó como inválida una corrida limpia.** Una corrida de `corre` sobre la tarea b8d0db4058 dio 6 eventos `oraculo_leido`, todos con motivo `huella`. La cita era una cabecera `diff --git` de un archivo de producción, en líneas del transcript donde el builder corrió `git diff`.

**La causa está en `HuellasOraculo::armar`,** en `src/observe/transcript.rs`.
- `armar` lee cada archivo de `oculto/` línea por línea.
- Una línea es huella si aparece una sola vez, si `es_huella_candidata` la acepta y si no está en el texto visible: el contrato y el `bootstrap`.
- `es_huella_candidata` pide 40 caracteres o más, y deja fuera los `import` de Python y los comentarios. No sabe nada de diffs.

**Una tarea con tests en línea lleva en `oculto/` un `tests.diff`.** Sus cabeceras —`diff --git a/x b/x`, `--- a/x`, `+++ b/x`— nombran archivos de producción. Pasan el filtro. Si el builder toca ese mismo archivo, su propio `git diff` imprime la misma cabecera. En 22 de las 33 tareas del dataset, los tests ocultos van en archivos de producción. Sin el arreglo, la medición D3 no se puede correr.

**Otras líneas del diff tampoco son huella real:**
- las cabeceras de hunk, `@@ -10,6 +10,20 @@ fn nombre()`;
- las líneas `index`, `new file mode`, `deleted file mode`, `similarity index`, `rename from` y `rename to`;
- las de contexto y las quitadas, que son código del padre.

**Un `use` largo de Rust tampoco lo es.** Un builder que hace bien la tarea escribe el mismo `use` para la misma API. Hoy los `import` de Python ya quedan fuera, y los `use` no.

**Las corridas viejas marcadas no se pueden revisar.** No hay un comando que arme las huellas y revise un transcript guardado.

## Tarea

La regla que manda: **una huella es una línea del test oculto que el builder no puede escribir por sí solo, nunca una cabecera ni una importación.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Las huellas de un diff y los `use` de Rust

1. **Un archivo de `oculto/` que termina en `.diff` o `.patch` se lee como diff.**
   - Las cabeceras no cuentan: `diff --git`, `index `, `--- `, `+++ `, `@@`, `new file mode`, `deleted file mode`, `similarity index`, `rename from` y `rename to`.
   - Las líneas de contexto, que empiezan con espacio, no cuentan.
   - Las líneas quitadas, que empiezan con `-`, no cuentan.
   - **Solo cuentan las líneas agregadas,** sin su `+` inicial.
2. **`es_huella_candidata` deja fuera los `use ` y los `pub use ` de Rust,** igual que los `import` de Python.
3. **El resto del filtro no cambia:** 40 caracteres o más, una sola vez, y fuera del texto visible.
4. **Un archivo que no es diff** se lee como hoy.

**Tests en rojo primero:**
- Unitario: con un `tests.diff` en `oculto/` y un transcript que trae la misma línea `diff --git`, no hay huella. Hoy la hay.
- Unitario: lo mismo con `--- a/x`, `+++ b/x` y una cabecera `@@` larga.
- Unitario: una línea agregada del diff, de 40 caracteres o más, que aparece en el transcript sin el `+`, sí es huella.
- Unitario: una línea de contexto larga del diff no es huella.
- Unitario: `use paquete::modulo::TipoConNombreLargo;` no es huella. Hoy lo es.
- Guarda: un archivo de test que no es diff da las mismas huellas que hoy.
- Guarda: los tests de `transcript.rs` de hoy siguen verdes.

### 2. El comando `corre revisar-lectura`

1. **`r00t corre revisar-lectura --tarea <tarea> --transcript <ruta>`.** Arma las huellas de la tarea con la regla nueva, con el `ROOT_HOME` vigente, y revisa el transcript con la misma función que usa `corre`.
2. **Imprime solo conteos:** cuántas lecturas por motivo, `huella` y `nombre`, y si el transcript queda marcado.
3. **Nunca imprime la cita ni la huella.** Una huella es una línea de un test oculto.
4. **Con `--json`,** da los mismos conteos en JSON, sin citas.
5. **Un transcript que no existe, o una tarea sin `oculto/`,** da un error de uso con la ruta.
6. **No escribe nada:** ni eventos ni filas.

**Tests en rojo primero:**
- Unitario: con un transcript que trae una huella real, el comando da 1 lectura por `huella`, y la salida no contiene la línea de la huella. Hoy el comando no existe.
- Unitario: con un transcript limpio, da 0 lecturas y "sin marcas".
- Unitario: un transcript inexistente da un error de uso con su ruta.
- Guarda: el comando no crea ni cambia ningún archivo del `ROOT_HOME` del test.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 166.
- `revisar-lectura` sobre el transcript real de la corrida de b8d0db4058. Debe dar 0 marcas.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Oráculo.** Ninguna salida del comando nuevo lleva líneas de tests ocultos, ids ni citas.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-166.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Qué líneas de un diff cuentan como huella**: la regla, con un ejemplo falso de cada caso.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
