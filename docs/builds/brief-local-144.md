# Brief de la Tarea 144 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-143`. Trae las Tareas 84 a 138, la 127b y la 143.
- La 139 a la 142 tocan otros archivos. No toques `src/oracle/contenedor.rs`, `src/oracle/descarga.rs`, `src/oracle/calibra/target.rs`, `tools/colab/`, `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs`, `src/oracle/colab.rs`, `src/proveedores/mod.rs` ni `src/main.rs`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-144`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, `tiempo_tests` alternado en los casos | `src/oracle/eficiencia.rs` |
| 2, la compilación contra su ancla | `src/oracle/eficiencia.rs` |
| 3, el control de carga | `src/oracle/eficiencia.rs` |

- Los tres puntos tocan el mismo archivo. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** El motivo del control de carga va en el campo de motivo que la medida ya tiene.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La calibración de Eficiencia de t1 a t3 con la cadena 110 a 138 corrió con la laptop cargada.** Las corridas guardadas lo muestran:
- En los casos 2 y 3 de t1, `tiempo_tests` del repo corre entre 0,71 y 0,99 s. La base corría cerca de 0,4 s.
- En el limpio 1 de t1, la compilación dio razón 2,01. En los casos de t1, el ruido de la compilación quedó entre 1,22 y 1,54.
- En t2, la compilación de los dos defectos dio razones de 1,27 y 1,05. Un lastre de memoria no cambia el tiempo de compilación. Los dos defectos se "detectaron" por eso.

**El método no vio la carga.**
- **La referencia sale de la base,** que corrió sin carga. Cada caso agrega una sola corrida ancla del lado inicial (Tarea 133).
- **Una ancla sola casi no mueve la mediana** de once corridas (Tarea 138). En el limpio 3 de t1, el ancla dio 0,75 s, igual que el repo: la máquina se había vuelto el doble de lenta. Pero la referencia siguió en la mediana de la base, cerca de 0,39 s. La razón dio 1,96, y el limpio quedó detectado.
- **La compilación del caso** se compara con la serie de la base, que corrió en otro momento.

**Los costos.** `tiempo_tests` dura cerca de 0,4 s por corrida. Correr los dos lados 10 veces cada uno cuesta unos 4 s más por caso. La compilación dura de 66 a 83 s. Por eso sigue con su ancla única.

## Tarea

La regla que manda: **en un caso de calibración, cada medida de tiempo se compara con una referencia medida en el mismo momento, y un caso con la máquina cargada no da veredicto.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. `tiempo_tests` alternado en los casos

1. **Con la referencia fijada** (`con_referencia`, Tarea 131), `tiempo_tests` y `memoria_pico` ya no usan la serie de la base.
2. **Cada caso corre los dos lados, alternados,** 10 veces cada uno: repo, inicial, repo, inicial, y así.
3. **La comparación y el ruido** siguen la regla de la Tarea 138: la mediana de cada lado, y 2 veces la mayor dispersión por cuartiles, con el piso de 2 %.
4. **El tamaño** sigue con la serie de la base, porque no cambia con el tiempo.
5. **Sin la referencia fijada,** la medición no cambia.

**Tests en rojo primero:**
- Unitario: con la referencia fijada, el ejecutor del lado inicial recibe 10 corridas de tests. Hoy recibe una.
- Unitario: con la referencia fijada y las corridas de los dos lados el doble de lentas que la base, la razón da cerca de 1, y no hay regresión. Hoy da cerca de 2.
- Unitario: el orden de las llamadas alterna los dos lados.

### 2. La compilación contra su ancla

1. **Con la referencia fijada,** la razón de `tiempo_compilacion` es la corrida del repo dividida por el ancla de la compilación. Las dos corrieron en el mismo caso.
2. **El ruido** sale de la serie de la base, con el piso de 2 %, como hoy.
3. **Si el ancla falla,** la medida queda sin valor, con el motivo, y el aviso de la Tarea 135 sigue.

**Tests en rojo primero:**
- Unitario: base de 66 s, ancla de 130 s y repo de 132 s. La razón es cerca de 1,015, y no hay regresión. Hoy la razón es cerca de 2.
- Unitario: base de 66 s, ancla de 66 s y repo de 132 s. La razón es 2, y hay regresión.

### 3. El control de carga

1. **Cada caso lee `/proc/loadavg`** antes y después de sus corridas medidas. La lectura corre en el host, no en el contenedor. La ruta se puede cambiar, para los tests.
2. **Si la carga de un minuto pasa la mitad de los núcleos** en alguna de las dos lecturas, todas las medidas de tiempo del caso quedan sin valor, con el motivo "la máquina estaba cargada: carga <x> con <n> núcleos". La memoria y el tamaño no cambian.
3. **Los núcleos** salen de `std::thread::available_parallelism`.
4. **El umbral de la mitad** va en una constante, con su motivo: la carga de otro trabajo dobló los tiempos de t1 en la calibración de la cadena 110 a 138.
5. **La base también** pasa por el control. Una base cargada deja la calibración sin veredicto, con el mismo motivo.

**Tests en rojo primero:**
- Unitario: un `loadavg` falso de `6.00 ...` con 8 núcleos deja `tiempo_tests` y `tiempo_compilacion` sin valor, con el motivo.
- Unitario: un `loadavg` falso de `1.00 ...` con 8 núcleos no cambia nada.
- Unitario: `memoria_pico` y `tamano_binario` siguen con valor en una máquina cargada.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 143 y la 144, en una ventana sin otro trabajo en la laptop. Los limpios deben dar 1,0, y ningún caso debe quedar sin veredicto por carga.

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

Al terminar, escribí `docs/builds/informe-local-144.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las llamadas de un caso**: cada llamada al ejecutor de cada lado, en orden, con cuántas corridas tiene.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
