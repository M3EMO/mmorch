# Brief de la Tarea 162b para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (79).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-162`. Trae las Tareas 84 a 162, con el punto 2 de la 162 hecho.
- **Arrancá cuando termine la suite con podman de la cadena 162.** No corras mientras el orquestador o el usuario usen podman.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-162b`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el informe de un corte de la descarga | `src/oracle/mod.rs` |
| 2, el plazo real en el motivo, y la guarda del texto | `src/colab_calibra.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/informe.rs`, `src/oracle/descarga.rs`, `src/oracle/colab.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **No cambies ninguna regla de decisión.** `decidir`, `bloqueos`, `no_listo` y `sin_decision` quedan como están. Qué decide Root ante un corte espera la pregunta P121 del usuario.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.** Los tests usan el `ColabCli` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**1. Un corte de la descarga sale sin informe.** `preparar_rust`, en `src/oracle/descarga.rs`, devuelve `Preparacion::SinVeredicto` en tres cortes: "no instala", "lock inválido" y "fuente no aceptada". `evaluar_con`, en `src/oracle/mod.rs`, vuelve enseguida con `sin_veredicto(...)`, y se salta `informe::informar_con`. El usuario recibe un veredicto con `informe` nulo: no ve por qué.

La 162 se detuvo en este punto. Su brief esperaba `Bloqueado`, pero `bloqueos` solo cuenta hallazgos de Seguridad, y el hallazgo del corte es de Entrega. Qué decisión corresponde lo decide el usuario en P121. **Esta tarea solo arma el informe, sin cambiar la decisión.** Con las reglas de hoy, la decisión va a ser `SinDecision`.

**2. El motivo de la última falla dice siempre "plazo de 180 s".** Usa `PLAZO_LOCAL_TRABAJO`, pero `lim.restante()` puede recortar el plazo real. Además, `motivo_de_error` reconoce el plazo por el texto "colab superó", que escribe `colab::paso`. Si ese texto cambia, el motivo pierde la palabra "plazo" sin que ningún test lo note.

## Tarea

La regla que manda: **todo veredicto lleva su informe, y un mensaje dice el valor que de verdad se usó.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El informe de un corte de la descarga

1. **El camino de `Preparacion::SinVeredicto` arma su informe** con `informe::informar_con`, igual que el camino normal, y fija `veredicto.estado` con `informe::estado_de`.
2. **Sin redactor.** El informe de un corte es determinista. No llama a ninguna API.
3. **La decisión sale de `decidir`, tal como está.** No agregues ni cambies ninguna regla.
4. **Las nueve dimensiones siguen sin veredicto,** con el motivo de hoy.
5. **El informe nombra el hallazgo del corte** y dice que las suites no corrieron, si las reglas actuales del informe lo permiten sin tocar `informe.rs`. Si no lo permiten, anotalo.
6. **Vale para los tres cortes.**

**Tests en rojo primero:**
- Unitario: una evaluación con un `Preparacion::SinVeredicto` de "no instala" da un veredicto con informe. Hoy el informe es nulo.
- Unitario: lo mismo con "lock inválido" y con "fuente no aceptada".
- Unitario: la decisión de los tres es la que da `decidir` hoy. Se espera `SinDecision`. Si da otra, anotala en el informe, sin cambiar reglas.
- Guarda: las nueve dimensiones siguen sin veredicto.
- Guarda: el camino normal no cambia.

### 2. El plazo real en el motivo, y la guarda del texto

1. **El motivo de una consulta que vence dice el plazo usado:** el menor entre `PLAZO_LOCAL_TRABAJO` y `lim.restante()`. Se calcula en `colab_calibra.rs`.
2. **Un test fija el texto que reconoce `motivo_de_error`.** Llama a `colab::paso` con el `ColabCli` falso, deja vencer el plazo, y comprueba que el motivo diga "plazo". Si alguien cambia el texto de `colab::paso`, el test falla.

**Tests en rojo primero:**
- Unitario: con `lim.restante()` de 50 segundos, el motivo dice "plazo de 50 s". Hoy dice "plazo de 180 s".
- Unitario: el motivo de un plazo vencido por `colab::paso` contiene "plazo".

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 162b.
- La verificación otra vez de b8d0db4058 con su lock real. Debe traer informe.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-162b.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La decisión de cada corte**: lo que da `decidir` hoy para los tres, y qué dice su informe.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
