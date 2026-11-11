# Brief de la Tarea 92 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-91` (9b47e65). Las Tareas 84 a 91 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-92`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, rutas absolutas en el envoltorio | `src/oracle/entrega.rs` |
| 2, sonda de red con control | `src/oracle/entrega.rs`, y los tests de `src/oracle/mod.rs` que usan la sonda |

- Además: `tests/entrega.rs`.
- Los dos puntos tocan `entrega.rs`. Van en serie: primero el 1.
- **No toques:** el resto de `src/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- **Podman, solo para la entrega.** Si el orquestador no corre otra suite, el organizador puede correr con podman `tests/entrega.rs` y los tests de `entrega` de la biblioteca, como en la 91b. Preguntale antes de empezar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

La re-revisión de la Tarea 91 sobre 9b47e65 no halló HIGH. Halló dos MEDIUM.

**MEDIUM 1, `sh` y `timeout` falsos.** `envoltorio_de` (`src/oracle/entrega.rs:1285`) llama a `timeout` y a `sh` después de activar el entorno. Esos nombres se resuelven con el `PATH` ya activado, con el venv de Root primero. `instalar` es código del producto. Puede dejar un `sh` o un `timeout` falso en `/tmp/venv-eval/bin` o en `/tmp/pydeps/bin`. Ese programa falso corre en cada escenario y puede salir con 0.

**MEDIUM 2, la sonda de red queda fija.** `red_disponible` (`entrega.rs:150`) guarda su resultado en un `OnceLock` por proceso. Una imagen sin `sh`, o un plazo vencido una sola vez, deja "sin red" para todas las tareas siguientes. Esas tareas quedan pendientes en vez de rojas.

## Tarea

La regla que manda: **el envoltorio solo llama programas del sistema, y una sonda que falla no cuenta como una medición.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Rutas absolutas en el envoltorio

1. Al arrancar la sesión, Root busca las rutas de `sh` y de `timeout` con el `PATH` del sistema, sin activar nada. Usá `podman exec <c> /bin/sh -c 'command -v sh; command -v timeout'`, o un chequeo equivalente con rutas fijas.
2. La sesión guarda esas dos rutas absolutas. `Sesion` ya guarda si hay `timeout`: guardá la ruta en vez del booleano.
3. `envoltorio_de` usa las rutas absolutas para el `sh` interno y para `timeout`.
4. Una ruta que no empieza con `/`, o que vive bajo `/tmp`, `/salida` o la carpeta de trabajo, se rechaza. Sin `sh` válido, la sesión no arranca, con su motivo. Sin `timeout` válido, la sesión sigue sin él, como hoy.

**Tests en rojo primero:**
- Unitario: el envoltorio de un paso con activación usa la ruta absoluta de `sh` y la de `timeout`.
- Unitario: una ruta de `sh` bajo `/tmp` se rechaza.
- Guarda: los tests de la 91c del orden del `PATH` siguen verdes.
- Con podman: un `instalar` que deja un `sh` falso que sale con 0 en `/tmp/venv-eval/bin` no hace pasar un escenario que debe fallar.

### 2. Sonda de red con control

1. **Tres resultados.** `red_disponible` pasa de `bool` a un tipo con tres casos: hay red, no hay red, y la sonda no pudo correr.
2. **Control.** Primero corre `podman run --rm --network none --entrypoint sh <imagen> -c true`. Si falla, la sonda no pudo correr.
3. **Prueba.** Después corre el mismo comando con `--network pasta:--no-map-gw`. Si el control pasó y la prueba falla, no hay red.
4. **Desenlaces en el build.**
   - Hay red: como hoy.
   - No hay red: pendiente, con el motivo "la red no se pudo configurar en este host", como hoy.
   - La sonda no pudo correr: infraestructura, con el motivo del control. No es pendiente de red.
5. **Memoria.** Se guarda por imagen, y solo el resultado "hay red". Los otros dos se vuelven a medir en el próximo build.
6. Las opciones de los dos `podman run` son las de hoy: `--pull=never` y sin el proxy del host.

**Tests en rojo primero:**
- Unitario: con el control fallando, el build da infraestructura, no pendiente.
- Unitario: con el control en verde y la prueba fallando, el build queda pendiente con su motivo.
- Unitario: una sonda que falló una vez y pasa la segunda vez da "hay red" en el segundo build.
- Unitario: "hay red" de una imagen no vale para otra imagen.
- Guarda: los tests de clase B de la 91b siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-92.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`, y los tests de entrega con podman si corrieron.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
