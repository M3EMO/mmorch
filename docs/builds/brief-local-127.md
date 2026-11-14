# Brief de la Tarea 127 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-125`. Trae las Tareas 84 a 125.
- La 126 se construye a la vez. Toca módulos de tests de varios archivos, `src/lib.rs`, un módulo nuevo y `tests/`. No toca `mutacion.rs` ni `calibra.rs`. No toques los archivos de su tabla.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-127`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la mutación usa una carpeta fija | `src/oracle/mutacion.rs` |
| 2, la calibración da la carpeta | `src/oracle/calibra.rs` |

- El punto 2 usa el punto 1. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/lib.rs`, `src/oracle/mod.rs`, `src/oracle/perfiles/herramientas.rs`, `src/tipos.rs`, los archivos de la 126, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La medición.** El orquestador calibró Pruebas de s2-t1 con n=4, con la Tarea 123 y un `podman` envoltorio.
- La base tardó 104 s con 14 mutantes. Cada caso tardó entre 78 y 82 s con 3 mutantes.
- Cada mutante cuesta unos 2,2 s. **El costo fijo de cada corrida es de unos 73 s.**

**La causa: cada corrida compila las dependencias desde cero.**
1. La mutación corre con `CARGO_TARGET_DIR=/salida/target` (`comando_de_corrida`, `src/oracle/mutacion.rs:695`).
2. Corre por `EjecutorContenedor::ejecutar` (`src/oracle/perfiles/herramientas.rs:89`). Esa función crea un `/salida` nuevo en cada llamada. En el registro del orquestador, cada corrida monta un `home/tmp/sondas-<azar>` distinto. El `target` siempre parte vacío.
3. El reuso del `target` de la Tarea 59 vale para las suites, no para la mutación.
4. Además, cargo-mutants copia el árbol a una carpeta temporal con nombre al azar. La ruta del crate cambia en cada corrida, y la compilación incremental no sirve entre corridas.

**La herramienta ya existe.** La Tarea 119 sumó `EjecutorHerramienta::ejecutar_en(cmd, salida)`. `EjecutorContenedor` monta la carpeta dada en `/salida`, y no la crea ni la borra. Un ejecutor falso la ignora y llama a `ejecutar`.

**Cómo pasa hoy el contexto de la calibración a la mutación.** Por valores de hilo: `con_plazo_fijo` (`src/oracle/mutacion.rs:83`) y `con_muestra_de_base` de la 123. `evaluar_copia` (`src/oracle/calibra.rs:1022`) los usa alrededor del evaluador.

## Tarea

La regla que manda: **en una calibración, la mutación compila las dependencias una vez, y todas sus corridas usan la misma ruta.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La mutación usa una carpeta fija

1. **Un valor de hilo nuevo** guarda una carpeta de trabajo de la mutación. Una función `con_carpeta_de_mutacion(carpeta, f)` la fija mientras corre `f`, y la restaura al salir, también si `f` entra en pánico.
2. **Sin ese valor,** la mutación corre como hoy. Una evaluación normal no cambia.
3. **Con ese valor,** el listado y la corrida usan `ej.ejecutar_en(cmd, carpeta)`.
4. **Con ese valor, el comando cambia:**
   - empieza con `rm -rf /salida/work`, antes de `cp -a /repo /salida/work`;
   - cargo-mutants lleva `--in-place`;
   - `CARGO_TARGET_DIR` sigue en `/salida/target`, que ahora persiste.
5. **El `rm -rf` corre dentro del contenedor,** sobre una carpeta que Root creó para esta calibración. No es un borrado del host. Un comentario lo dice, y nombra la regla de `src/borrado.rs`.
6. **Los archivos que lee Root** (`outcomes.json`, `tests-fallidos.txt` y el listado) se leen de la carpeta fija. Cada corrida los reescribe antes de leerlos. Un archivo viejo de otra corrida no se debe leer: borralos al principio del comando.

**Tests en rojo primero, con un ejecutor falso que anota la carpeta y el comando:**
- Unitario: con la carpeta fija, el listado y la corrida reciben esa carpeta. Hoy no reciben ninguna.
- Unitario: con la carpeta fija, el comando trae `rm -rf /salida/work`, `--in-place` y `CARGO_TARGET_DIR=/salida/target`.
- Unitario: con la carpeta fija, el comando borra `outcomes.json` y `tests-fallidos.txt` antes de correr cargo-mutants.
- Guarda: sin la carpeta fija, el comando es el de hoy.

### 2. La calibración da la carpeta

1. **La calibración de una dimensión con mutación** crea una carpeta dentro de `home/tmp`, con el helper de temporales de siempre.
2. **La base y todos los casos** corren dentro de `con_carpeta_de_mutacion` con esa carpeta.
3. **La carpeta se borra al terminar la calibración,** también en un error, por `src/borrado.rs`.
4. **Una calibración sin mutación** no crea la carpeta.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: la base y dos casos reciben la misma carpeta de mutación.
- Unitario: la carpeta no existe después de la calibración, también si un caso devuelve error.
- Unitario: una calibración de Completitud no crea la carpeta.
- Guarda: los tests de calibración de Pruebas de las Tareas 84 a 123 siguen verdes, salvo los que fijaban el comando viejo. Anotá esos en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Los tests de calibración de Pruebas con `saltar_sin_podman`.
- Que `--in-place` en `/salida/work` dé los mismos mutantes y estados que hoy.
- **La medición de la ganancia:** el orquestador repite el reparto del tiempo de la vuelta (42), sección 6, con el binario de la 127. Cada caso de Pruebas debe bajar de unos 80 s a unos 20 s. La sensibilidad y la especificidad deben dar lo mismo que en `reparto2`.
- Que la carpeta de mutación desaparezca de `home/tmp` al terminar.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root en el host pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-127.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El comando nuevo**: el texto exacto del listado y de la corrida con la carpeta fija.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
