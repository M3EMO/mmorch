# Brief de la Tarea 160 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (76).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-159`. Trae las Tareas 84 a 159 y la 127b.
- Ningún otro builder corre a la vez. No corras mientras el orquestador use podman.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-160`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la regresión de shell | `src/oracle/mod.rs` |
| 2, el nombre del test desde el junit | `src/oracle/junit.rs`, `src/oracle/mod.rs` |
| 3, la versión de las sondas de Seguridad | `src/oracle/previos.rs` |

- Los puntos 1 y 2 comparten `mod.rs`. Van en serie: primero el 1. El punto 3 corre en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **Los 4 tests de inestables de `tests/oraculo.rs` no se editan.** Son la guarda del punto 1.
- **No toques:** `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** El nombre del test se guarda solo en memoria.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**1. La 159 rompió los reintentos de los lenguajes que no son python ni rust.** La suite con podman de la cadena 110 a 159 dio 4 fallas en `tests/oraculo.rs`, en los tests de inestables. Dan `Tarea("el reintento no tiene regla para el lenguaje shell")`. Antes de la 159, ese lenguaje usaba la plantilla de `reintento`, o la cola del comando. El brief de la 159 pedía "el error de hoy" para otro lenguaje. Ese error solo salía sin plantilla ni cola. Debió pedir "la conducta de hoy".

**2. `termino_nextest` separa el id en el primer `::`, y eso no sirve.** `junit::id_de` arma el id como `<classname>::<nombre>`. En nextest, el `classname` es el id del binario. Los dos lados pueden traer `::`:
- un test de integración tiene un binario `paquete::archivo`, y su nombre queda mal;
- un paquete sin lib da `binary_id(=paquete)`, que no coincide con ningún binario.

El orquestador probó nextest 0.9.100 en la imagen rust:4. `binary_id(=x)` sin coincidencia es un error de parseo, y la corrida entera falla. `test(=x)` sin coincidencia no es error: elige 0 tests.

**La regla que eligió el planner:** el nombre se guarda al leer el junit, que trae el binario y el nombre por separado, y el filtro usa solo `test(=<nombre>)`. Un test con el mismo nombre en otro binario también corre. Eso solo suma resultados, y la clasificación usa el id completo.

**3. La 159 cambió una sonda de Seguridad, y su versión no subió.** `configs-secretos` ya no marca `read`, `write` ni `none` como secretos. Una evidencia de Seguridad guardada con la versión 3 puede traer ese falso positivo. `VERSION_SONDAS_SEGURIDAD`, en `src/oracle/previos.rs`, vale 3. El builder de la 159 no la subió, porque el archivo estaba fuera de su lista.

## Tarea

La regla que manda: **un arreglo para Rust no cambia la conducta de los otros lenguajes, y un id no se corta adivinando.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La regresión de shell

1. **Solo Rust usa la regla nueva.** Cualquier otro lenguaje, python y shell incluidos, vuelve a la conducta de antes de la 159.
2. **La conducta de antes:** usa la plantilla de `reintento` si existe, con los ids como los armaba `node_id`. Si no hay plantilla, usa la cola del comando. El error sale solo sin plantilla ni cola, con el texto de antes de la 159.
3. **Leé la conducta de antes** en la punta de `laptop/tarea-158`, función `reintento_de` de `src/oracle/mod.rs`. Citá el commit en el informe.

**Tests en rojo primero:**
- Unitario: una tarea de lenguaje `shell` con un comando que termina en el directorio de la suite da la cola del comando con los ids. Hoy da "el reintento no tiene regla para el lenguaje shell".
- Guarda: los 4 tests de inestables de `tests/oraculo.rs` vuelven a verde sin cambios. Con `ROOT_SIN_PODMAN=1` pueden quedar ignorados. Anotá si corren en tu suite.
- Guarda: una tarea Python da los mismos node ids que antes de la 159.

### 2. El nombre del test desde el junit

1. **Al leer el junit, Root anota, para cada id, el `name` del `testcase`.** Va en memoria, junto a la clasificación de la suite. No va a ningún archivo.
2. **`reintento_de`, en Rust con plantilla,** reemplaza `{tests}` por `test(=<nombre1>) | test(=<nombre2>) | ...`, entre comillas simples para `sh`. Sin `binary_id`.
3. **Un id sin nombre anotado** no se corta. Esa corrida extra repite el comando entero, como una tarea Rust sin `reintento`.
4. **`termino_nextest` sale,** o deja de cortar el id. Uní sus llamadores con codegraph y `grep -w` antes de cambiarla.
5. **Un nombre con caracteres especiales para nextest** se escapa como pide la sintaxis de filtros de nextest. Citá la regla en el informe.

**Tests en rojo primero:**
- Unitario: un junit con `classname="paquete::archivo"` y `name="modulo::test_a"`, marcado inestable, da `test(=modulo::test_a)`. Hoy da `binary_id(=paquete) & test(=archivo::modulo::test_a)`.
- Unitario: el filtro de Rust nunca contiene `binary_id`.
- Unitario: un inestable sin nombre anotado da el comando entero.
- Guarda: una tarea Rust sin `reintento` sigue dando el comando entero.

### 3. La versión de las sondas de Seguridad

1. **`VERSION_SONDAS_SEGURIDAD` sube de 3 a 4.**
2. **El comentario de la constante** suma una línea: "4: `configs-secretos` deja de marcar `read`, `write` y `none` (Tarea 159)".
3. **Revisá qué hace Root con un previo de una versión vieja,** y citá esa regla en el informe.

**Test en rojo primero:**
- Unitario: un previo de Seguridad guardado con la versión 3 no se reusa con la versión actual. Hoy la versión actual es 3, y se reusa.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 160. Los 4 tests de inestables de `tests/oraculo.rs` deben pasar.
- La verificación otra vez de las 4 tareas de watchexec y de b8d0db4058 con su lock real.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-160.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El camino del nombre**: desde el junit hasta `reintento_de`, struct por struct.
5. **La regla de los previos viejos**: qué hace Root con un previo de otra versión, y dónde.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
