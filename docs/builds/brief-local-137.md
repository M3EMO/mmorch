# Brief de la Tarea 137 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- La 138 se construye a la vez. Toca solo `src/oracle/eficiencia.rs`. No toques ese archivo.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-137`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, marcar la copia y leer la lista | `src/oracle/calibra/sonda_lastre.rs` (nuevo), `src/oracle/calibra.rs` (solo la línea `mod sonda_lastre;`) |
| 2, correr la sonda en la base | `src/oracle/mod.rs`, `src/oracle/calibra/sonda_lastre.rs` |
| 3, pasar la lista al inyector | `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/eficiencia.rs`, `src/lib.rs`, `src/tipos.rs`, `src/oracle/perfiles/`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** Si un punto lo pide, detenete y escribilo en el informe.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El defecto `LastreDeMemoria` no se ve en la medición.** En la última calibración de Eficiencia de t1 a t3, la especificidad dio 1,0 en las tres tareas, y la sensibilidad dio 0 en las tres. Ningún lastre movió la memoria pico: las razones quedaron entre 0,997 y 1,009.

**La medición solo corre los tests comunes.** Son los tests que pasan en `bootstrap/` y en el repo. Un lastre en una función que esos tests no corren no se puede ver.

**Las dos reglas por texto fallaron:**
- **La 134** eligió una función que existía en `bootstrap/`. Existir no quiere decir que un test la corra.
- **La 136** eligió una función cuyo nombre aparece llamado en un test viejo. `new` y `from` aparecen en casi cualquier test. Los lastres cayeron en `fn new` de `content_pattern.rs:28`, en `fn from` de `hex/byte.rs:19`, y en un constructor de `mount_space_display.rs:33`.
- En la vuelta (49), t2 sí vio sus dos lastres en `content_search/mod.rs:32` y `:44`, con razón 2,4. Ahí cayeron en funciones que los tests corren.

**Un nombre no prueba que una función corra. Correrla sí.** Esta tarea corre los tests viejos sobre una copia marcada de `bootstrap/`, y anota qué funciones viejas corrieron.

**Por qué en `bootstrap/` y no en el repo.** En el repo también corren los tests nuevos. Marcarían funciones que los tests comunes no tocan. En `bootstrap/` solo hay tests viejos, que son casi todos los comunes.

**Por qué en la evaluación de la base, dentro de `mod.rs`.** La compilación sin red necesita la caché de dependencias, que prepara `evaluar_con`. La línea base de Seguridad ya corre sondas sobre `bootstrap/` con la fábrica `deps.herramientas` (`previos::linea_base`, cerca de la línea 798 de `src/oracle/mod.rs`). La sonda sigue ese camino, y los tests usan la fábrica falsa de siempre.

## Tarea

La regla que manda: **el lastre va a una función vieja que los tests viejos de verdad corren, según una corrida medida.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Marcar la copia y leer la lista

1. **Una función pura marca un archivo de Rust.** Al principio del cuerpo de cada función vieja de producción, inserta:
   ```rust
   { static ROOT_SONDA_<n>: ::std::sync::atomic::AtomicBool = ::std::sync::atomic::AtomicBool::new(false); if !ROOT_SONDA_<n>.swap(true, ::std::sync::atomic::Ordering::Relaxed) { let _ = ::std::fs::write("/tmp/root-sonda/<id>", b""); } }
   ```
   `<n>` es un número único en el archivo. `<id>` es un identificador fijo de la función: el archivo relativo, el número de línea y el nombre, con caracteres seguros para un nombre de archivo.
2. **La bandera hace que el archivo se cree una sola vez.** Una función que corre muchas veces no hace más lentos los tests.
3. **Una función vieja de producción** sigue las reglas del lugar de `LastreDeMemoria`: no vale en una función `const`, en un bloque `unsafe`, en código de tests ni en un `main` de ejemplo. En `bootstrap/`, toda función de producción es vieja.
4. **Una función pura arma la copia marcada** de `bootstrap/` en una carpeta, y devuelve el mapa de `<id>` a archivo, línea y nombre.
5. **Una función pura arma el comando:** crea `/tmp/root-sonda`, corre los tests de la copia con nextest, sin cortar en el primer fallo, y lista los archivos de `/tmp/root-sonda`. Seguí la forma de los comandos de nextest de `src/oracle/eficiencia.rs`, sin tocar ese archivo.
6. **Una función pura lee la salida** y da el conjunto de funciones que corrieron.

**Tests en rojo primero:**
- Unitario: marcar un archivo con dos funciones deja dos marcas con números distintos. El archivo marcado compila con `cargo check --offline`.
- Unitario: una función `const` y una función de tests no reciben marca.
- Unitario: la lectura de una salida con dos `<id>` da las dos funciones.
- Unitario: un `<id>` que no está en el mapa se ignora.

### 2. Correr la sonda en la base

1. **Un valor de hilo** en `sonda_lastre.rs` dice si la evaluación debe correr la sonda. Una función `con_sonda_lastre(f)` lo fija mientras corre `f`. Otra, `tomar_sonda_lastre()`, devuelve el resultado, como `tomar_muestra()` de la mutación.
2. **En `evaluar_con`,** si el valor está fijado, la tarea es de Rust y hay `bootstrap/`, Root arma la copia marcada en una carpeta temporal del `home`, y corre el comando con un ejecutor de `deps.herramientas` sobre esa copia. Usa la misma caché que la medición de Eficiencia.
3. **La carpeta se borra al terminar,** también en un error, por `src/borrado.rs`.
4. **Si la sonda falla** (la copia no compila, el comando sale mal o no hay salida), el resultado es un error con su motivo, y el veredicto suma un aviso. La evaluación sigue.
5. **Sin el valor fijado,** la evaluación no cambia.

**Tests en rojo primero, con la fábrica falsa de ejecutores:**
- Unitario: con el valor fijado, la evaluación llama al ejecutor con el comando de la sonda, sobre una copia de `bootstrap/`.
- Unitario: sin el valor, no lo llama.
- Unitario: con una salida falsa de la sonda, `tomar_sonda_lastre()` da las funciones de esa salida.
- Unitario: un ejecutor que falla deja un aviso, y la evaluación sigue.
- Unitario: la carpeta de la copia no existe después de la evaluación.

### 3. Pasar la lista al inyector

1. **La calibración de Eficiencia** corre la base dentro de `con_sonda_lastre`, y toma el resultado después.
2. **La base de otras dimensiones** no corre la sonda.
3. **El inyector recibe la lista** por el contexto que ya usa. Sumá un campo con valor por defecto vacío. Si el contexto vive en un archivo fuera de la tabla, detenete y escribilo en el informe.
4. **`LastreDeMemoria` elige,** en orden:
   - una función de la lista en un archivo del cambio;
   - una función de la lista en cualquier archivo de producción;
   - si la sonda falló o no trae ninguna, la regla de la 136.
5. **La función elegida en el repo** es la que tiene el mismo archivo relativo y el mismo nombre que la de la lista.
6. **Si la lista trae funciones pero ninguna existe en el repo,** el defecto queda sin lugar, con el motivo de la Tarea 135.

**Tests en rojo primero:**
- Unitario: con una lista que trae una sola función vieja, el lastre va a esa función. Hoy va a la que elige la 136.
- Unitario: con la lista vacía, el lastre usa la regla de la 136.
- Unitario: una calibración de Eficiencia con un evaluador falso corre la base dentro de `con_sonda_lastre`. Una calibración de Pruebas no.
- Guarda: los tests de `LastreDeMemoria` de la 131, la 134 y la 136 siguen verdes, salvo los que fijaban el lugar viejo. Anotá esos en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **Que la copia marcada de `bootstrap/` compile y corra** en la imagen, para t1, t2 y t3.
- **El costo de la sonda:** su tiempo, con el envoltorio de podman de la vuelta (42), sección 6. Se estima en 2 a 4 minutos por tarea.
- **Cuántas funciones viejas marca la sonda como corridas** en cada tarea, y si `content_search/mod.rs` de t2 está entre ellas.
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 137 y la 138. Los lastres deben subir la memoria pico.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root en el host pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host. La carpeta `/tmp/root-sonda` vive dentro del contenedor.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-137.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La marca y el comando**: un ejemplo de función marcada, y el texto exacto del comando de la sonda.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
