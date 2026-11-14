# Brief de la Tarea 149 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (61).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-147`. Trae las Tareas 84 a 147 y la 127b.
- La 148 se construye a la vez. Toca `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs` y `src/oracle/calibra.rs`. No toques esos archivos, salvo tests que lean un literal que esta tarea cambia.
- La 150 se construye después. Toca `src/colab_calibra.rs`, `src/oracle/colab.rs`, `src/lib.rs` y `tools/colab/verificar-versiones.sh`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-149`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el guion `root-medir.py` | `src/oracle/eficiencia.rs` |
| 2, la memoria por proceso | `src/oracle/eficiencia.rs` |
| 3, la carga ajena con `getrusage` | `src/oracle/eficiencia.rs` |

- Los tres puntos comparten archivo. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe. En `mod.rs` y `calibra.rs`, tocá solo esos tests: la 148 trabaja ahí.
- **No toques:** los archivos de producción de la 148 y la 150, `src/oracle/contenedor.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** La medida conserva su id, `memoria_pico`.
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

**Eficiencia se calibra en Colab, y Colab no tiene cgroup por corrida.** En el runtime, `/sys/fs/cgroup` es de solo lectura. bwrap no crea un cgroup. El usuario decidió P111 b: `memoria_pico` pasa a ser el máximo de memoria de un solo proceso.

**Hoy dos medidas dependen del cgroup.**
1. `cmd_tests` imprime `ROOT_MEM` con `/sys/fs/cgroup/memory.peak`. En bwrap, ese archivo es el pico del runtime entero, desde su arranque. Daría un número falso sin aviso.
2. El control de carga lee `/proc/loadavg`. Esa carga cuenta los procesos de Root. En la v5 dio 14,08 justo después de las compilaciones `--release`. La base pasó ese motivo a todos los casos, y ningún tiempo tuvo valor.

**La salida: `getrusage` en vez del cgroup.** Un proceso que espera a sus hijos conoce su CPU y su mayor pico de memoria. Eso funciona igual dentro de podman y dentro de bwrap. `python3` está en la imagen del oráculo y en el runtime de Colab.

## Tarea

La regla que manda: **cada medida sale de los procesos que Root lanzó, nunca de la máquina entera.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El guion `root-medir.py`

1. **El texto del guion vive en `eficiencia.rs`,** como constante. Root lo escribe en la carpeta de salida de cada lado, antes de sus corridas medidas.
2. **Modo `cpu`:** `python3 /salida/root-medir.py cpu <comando>`. Lanza el comando con `sh -c`, espera su fin y sale con su código. Al final imprime `ROOT_CPU_US <n>`. `n` es la CPU de usuario y de sistema de `RUSAGE_CHILDREN` más la de `RUSAGE_SELF`, en microsegundos.
3. **Modo `rss`:** `python3 /salida/root-medir.py rss <binario> <args...>`. Lanza el binario, espera su fin y sale con su código. Agrega a `/salida/root-rss.lista` una línea con `ru_maxrss` de `RUSAGE_CHILDREN`, en bytes. En Linux, `ru_maxrss` viene en KiB.
4. **El modo `rss` no imprime nada en stdout.** nextest lee la salida de cada test.

**Tests en rojo primero:**
- Unitario: el guion, corrido con `python3` en un temporal del test, imprime `ROOT_CPU_US` con un número mayor que cero para `sh -c 'i=0; while [ $i -lt 100000 ]; do i=$((i+1)); done'`. Si el test no puede correr `python3`, que lo diga y quede ignorado con su motivo.
- Unitario: el modo `rss` con `python3 -c "b=bytearray(100*2**20)"` deja una línea de 100 MiB o más.
- Unitario: los dos modos salen con el código del comando.

### 2. La memoria por proceso

1. **La memoria corre en sus propias corridas,** separadas de las de tiempo. Son `CORRIDAS_MEMORIA = 4` por lado, alternadas como las de tiempo. Cuatro es `MIN_CORRIDAS_CUARTILES`.
2. **Cada corrida de memoria** borra `/salida/root-rss.lista`, fija `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="python3 /salida/root-medir.py rss"` y corre los tests comunes con el filtro.
3. **Al final, el comando imprime `ROOT_MEM`** con el mayor valor de la lista. La marca conserva su nombre.
4. **Las corridas de tiempo no usan el runner.** El arranque de Python cambiaría `tiempo_tests`. Su comando ya no imprime `ROOT_MEM`.
5. **Una lista vacía o sin número** deja `memoria_pico` sin valor, con el motivo "sin medida de memoria por proceso". Ese motivo reemplaza a "sin memory.peak".
6. **`/sys/fs/cgroup/memory.peak` sale** de todo comando.
7. **La documentación de `Medida::MemoriaPico`** dice qué mide ahora: el mayor pico de un proceso de test, sin `cargo` ni `nextest`.
8. **Revisá que nextest aplique el runner** de la variable de entorno. Citá en el informe la documentación de nextest que lo dice. Si no lo aplica, detenete y anotalo.

**Tests en rojo primero:**
- Unitario: el comando de memoria fija el runner y no lee `memory.peak`. Hoy lee `memory.peak`.
- Unitario: el comando de tiempo no fija el runner.
- Unitario: con una salida sin número en `ROOT_MEM`, la memoria queda sin valor con el motivo nuevo.
- Guarda: los tests de comparación de memoria por medianas siguen verdes.

### 3. La carga ajena con `getrusage`

1. **Cada corrida medida corre bajo el modo `cpu`:** las de tiempo de tests y cada compilación medida.
2. **Root lee la línea `cpu` de `/proc/stat`** justo antes y justo después de cada corrida medida. Mide también el reloj de esa corrida.
3. **La CPU ocupada de la máquina** es la suma de `user`, `nice`, `system`, `irq`, `softirq` y `steal`. Las unidades son de 1/100 s. Escribí ese 100 como constante con su motivo.
4. **La carga ajena de una corrida** es `(CPU ocupada − ROOT_CPU_US) / reloj`, en núcleos. Un valor negativo cuenta como cero.
5. **Un caso queda cargado** si alguna corrida medida supera `FRACCION_DE_NUCLEOS_CARGADA` por los núcleos. Sus tiempos quedan sin valor. El motivo dice "la máquina tenía carga ajena: X núcleos de N".
6. **Sin `ROOT_CPU_US` en una corrida,** los tiempos del caso quedan sin valor, con "sin medida de CPU propia". Nunca se supone carga cero.
7. **Cada caso suma un aviso** con su mayor carga ajena, aunque no esté cargado. Si el aviso no llega a la fila sin cambio de esquema, anotalo.
8. **La base sigue pasando su motivo** a los casos, como hoy.
9. **La lectura de `/proc/loadavg` sale,** con `leer_carga` y `motivo_de_carga` si quedan sin uso.
10. **`ROOT_LOADAVG` de la 147 pasa a `ROOT_PROC_STAT`.** Sin la variable, la ruta es `/proc/stat`. Los tests de `tests/` que llaman a `medir` la usan como hoy usan `ROOT_LOADAVG`.
11. **El control sigue apagado** en los tests del crate sin fuente fijada, como hoy.
12. **Revisá el memo y el reuso de sondas.** Una corrida medida servida desde un memo no tiene ventana real. Anotá si alguna puede pasar por el memo.

**Tests en rojo primero:**
- Unitario: una corrida con la CPU de la máquina igual a `ROOT_CPU_US` no queda cargada, aunque use todos los núcleos. Hoy, una carga de un minuto de 14 la deja sin valor.
- Unitario: una corrida con 6 núcleos ajenos de 8 deja sin valor los tiempos, con el motivo de carga ajena.
- Unitario: con 2 núcleos, una corrida con 1,5 núcleos ajenos queda cargada.
- Unitario: una corrida sin `ROOT_CPU_US` deja sin valor los tiempos, con "sin medida de CPU propia".
- Guarda: `memoria_pico` y `tamano_binario` no cambian con la carga.
- Guarda: un caso hereda el motivo de una base cargada.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 148, la 149 y la 150. Ahí se ve si nextest aplica el runner dentro del contenedor.
- La calibración de Eficiencia en Colab. Ahí se ve el ruido de la memoria por proceso y la carga ajena de un runtime.

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

Al terminar, escribí `docs/builds/informe-local-149.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lo que se borró**: cada función, constante o variable, con sus llamadores antes de borrar.
5. **El runner de nextest**: la cita de su documentación, y cualquier duda.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
