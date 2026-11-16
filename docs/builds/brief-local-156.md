# Brief de la Tarea 156 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (68).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-155`. Trae las Tareas 84 a 155 y la 127b.
- **Arrancá cuando el orquestador te pase el resultado de la Prueba 0f.** Ese resultado decide si el punto 1 se hace.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-156`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las CPU que Root no puede usar | `src/oracle/eficiencia.rs` |
| 2, el aviso de carga ajena en el caso | `src/oracle/eficiencia.rs`, y el lugar de la fila del caso que lo reciba |

- Los dos puntos comparten archivo. Van en serie, en este orden.
- **El punto 2 puede pedir otro archivo,** por ejemplo `src/oracle/calibra.rs`, donde se arma el renglón del caso. Antes de tocarlo, anotá en el informe qué archivo y por qué. Solo ese archivo, y solo para llevar el aviso.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** Si el aviso no cabe en un campo que ya existe, detenete y escribilo en el informe.
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

**La v6 de Eficiencia corrió en Colab, en tres runtimes de 2 núcleos.** En t1 y t3, casi todos los casos perdieron sus tiempos con "la máquina tenía carga ajena: 1.00 núcleos de 2", de 1,00 a 1,02. En t2, los tiempos tuvieron valor.

**Cómo mide Root la carga ajena hoy.** `Vigilancia::correr` lee la línea `cpu ` de `/proc/stat` antes y después de cada corrida medida. Le resta `ROOT_CPU_US`, la CPU propia que mide `root-medir.py` con `getrusage`. Divide por el reloj. El umbral es la mitad de `available_parallelism`.

**Una causa posible: la VM tiene más CPU de las que Root puede usar.** La línea `cpu ` suma todas las CPU de la VM. `available_parallelism` cuenta solo las que permiten el cgroup o la afinidad. Un servicio de Colab en una CPU extra cuenta como carga ajena, aunque no compite con la corrida. El valor fijo de 1,00 apunta a eso. La Prueba 0f del orquestador lo confirma o lo descarta.

**El aviso de mayor carga ajena no llega al caso.** La 149 suma a `avisos` de `medir` la línea "eficiencia: mayor carga ajena de una corrida: X núcleos de N". Los casos de la v6 no la traen en ningún campo. Sin ese dato, el umbral no se puede revisar.

## Tarea

La regla que manda: **la carga ajena cuenta solo lo que compite con la corrida, y su valor queda en el caso.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Las CPU que Root no puede usar

**Hacelo solo si la Prueba 0f muestra más CPU en `/proc/stat` que `available_parallelism`.** Si no, no lo hagas, y anotalo en el informe.

1. **Contá las CPU visibles:** las líneas `cpuN` de la misma fuente de `/proc/stat`. Llamalas `v`.
2. **Las CPU que Root puede usar** siguen siendo `available_parallelism`. Llamalas `n`.
3. **La carga ajena efectiva** es `max(0, ajena − (v − n))`. Un servicio en una CPU que Root no puede usar no compite con la corrida.
4. **Con `v = n`, nada cambia.** Es el caso de la laptop.
5. **Con `v < n`,** algo anda mal en la lectura. La corrida no tiene control, con un motivo que da `v` y `n`.
6. **El comentario** explica la resta y cita la v6: "1.00 núcleos de 2" en t1 y t3.
7. **`ROOT_PROC_STAT` sigue sirviendo** para los tests. El archivo falso puede traer líneas `cpuN`.

**Tests en rojo primero:**
- Unitario: con `v = 4`, `n = 2`, y un núcleo ajeno fijo, la corrida no queda cargada. Hoy queda cargada con "1.00 núcleos de 2".
- Unitario: con `v = 4`, `n = 2`, y 3,5 núcleos ajenos, la corrida queda cargada.
- Unitario: con `v = 2` y `n = 4`, la corrida no tiene control, con un motivo que da los dos números.
- Guarda: con `v = n = 8`, los tests de carga de la 149 siguen verdes.

### 2. El aviso de carga ajena en el caso

1. **Buscá dónde se arma el renglón de un caso** de la calibración, y qué campo de texto ya existente puede llevar el aviso. Por ejemplo, un `motivo` o una lista de avisos del caso.
2. **El renglón de cada caso de Eficiencia lleva su mayor carga ajena,** aunque no haya pasado el umbral.
3. **Sin cambio de esquema.** Si ningún campo existente sirve, no lo agregues. Detenete y anotá las opciones en el informe.

**Tests en rojo primero:**
- Unitario: un caso de Eficiencia con una carga ajena de 0,30 núcleos guarda ese valor en su renglón. Hoy no lo guarda.
- Guarda: un caso de otra dimensión no cambia.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 156.
- Si se hizo el punto 1, la v6 otra vez en Colab, con tres runtimes.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-156.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo. Si no hiciste el punto 1, decí qué mostró la Prueba 0f.
4. **Dónde quedó el aviso**: el campo del renglón, y un ejemplo.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
