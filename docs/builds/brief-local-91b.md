# Brief de la Tarea 91b para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Seguí en el mismo worktree, `~/.cache/root-wt/local91`, rama `laptop/tarea-91`, sobre la punta ea76273.
- Es la corrección de la Tarea 91. La suite con podman dio 14 fallas, y una revisión halló 3 HIGH.
- No uses `isolation: "worktree"` para tus subagentes.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta del worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el contenedor de evaluación no arranca | `src/oracle/entrega.rs`, `src/oracle/contenedor.rs` |
| 2, envoltorio del 125 y `PATH` | `src/oracle/entrega.rs` |
| 3, red por experimento | `src/oracle/entrega.rs` |
| 4, escenarios que piden red | `src/oracle/entrega.rs`, `src/oracle/tarea.rs`, `docs/interfaces.md` |

- Además, en todos los puntos: `tests/entrega.rs` y los tests de `src/oracle/mod.rs` que llaman a la entrega.
- Los cuatro puntos tocan `entrega.rs`. Van en serie, en el orden 1, 2, 3 y 4.
- **No toques:** el resto de `src/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- **Podman, solo para la entrega.** El organizador puede correr con podman los tests de `tests/entrega.rs` y los de `entrega` de la biblioteca. El orquestador no corre otra suite mientras tanto. Por ejemplo: `env -u ROOT_SIN_PODMAN tools/test-seguro.sh --test entrega` y `env -u ROOT_SIN_PODMAN tools/test-seguro.sh --lib entrega`.
- El resto del trabajo y los commits van con `ROOT_SIN_PODMAN=1`. La suite completa con podman la corre el orquestador al terminar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en el worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Las fallas, en `/home/mpera/.cache/root-wt/orquestador/revision-91.md` y `prueba91.out`.**

**Clase A, 11 tests.** El contenedor de evaluación no arranca: "la imagen no trae sleep ni tail". Las imágenes de los tests traen `sleep`. Causa probable, leída en el código y sin verificar:
- `iniciar_evaluacion` arma `podman run -d` con `argumentos_separado` (`src/oracle/contenedor.rs:124`). Monta `/repo`, `/salida`, la caché y `escenarios/`.
- El camino viejo, `ejecutar`, llamaba a `validar_rutas` (`contenedor.rs:401`) antes de lanzar. El nuevo no la llama.
- `Sesion::iniciar` (`src/oracle/entrega.rs:1275`) descarta el stderr de un `run` fallido, y termina con un mensaje que culpa a `sleep`.
- Si un montaje no existe, como la caché de `descarga::cache_de` o la carpeta de `escenarios/`, podman falla, y la causa queda oculta.

**Clase B, 3 tests.** Un `RUN wget` sin red da rojo enseguida, sin repetir con red. La regla 3 de la Tarea 91 prohibió creer el texto del repo, pero el diseño no dijo cómo saber que un fallo fue por la red.

**HIGH-1.** `pide_red` en `correr_escenario` (`entrega.rs:1384`) mira la salida de un escenario fallido. El producto imprime "could not resolve host", sale con 1, y el escenario queda pendiente.

**HIGH-2.** El producto puede salir con 125 en `instalar` o en un escenario. Root lo toma como error de podman (`entrega.rs:1374`, `:1737` y `:1760`), y no como rojo.

**HIGH-3.** `red_no_configurada` lee la última línea del stderr, `Error: building at STEP "<RUN>": ...`. Esa línea trae el texto del `RUN`, que es texto del repo. Un `RUN echo "setup network"; false` hace que el build con red quede pendiente en vez de rojo.

**MEDIUM-1.** `instalar` es código del producto. Puede dejar un `curl` falso en un directorio del `PATH`, y un escenario que usa `curl` pasa.

## Tarea

La regla que manda: **saber que un fallo es de red sale de un experimento, nunca de un texto; y un código de salida dice una sola cosa.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El contenedor de evaluación no arranca (clase A)

1. **El error real.** `Sesion::iniciar` conserva el stderr de cada `run` fallido. Su error lo incluye, con las dos pruebas de `sleep` y de `tail`.
2. **Rutas.** `iniciar_evaluacion` llama a `validar_rutas` antes del `run`. Hacela `pub(crate)` si hace falta.
3. **La causa.** Con el mensaje real, corré con podman los tests de la clase A. Arreglá la causa. Si no es un montaje, informala con el texto de podman.

**Tests en rojo primero:**
- Unitario: con un `Exec` falso cuyo `run` falla con "no such file or directory", el error de `Sesion::iniciar` trae ese texto.
- Unitario: una `Ejecucion` con una caché que no existe da un error que nombra esa ruta, sin llamar a podman.
- Con podman: los 11 tests de la clase A quedan verdes.

### 2. Envoltorio del 125 y `PATH`

1. **Envoltorio.** Cada paso corre así: `sh -c '<cmd>'; s=$?; [ "$s" -eq 125 ] && s=1; exit "$s"`. El envoltorio lo arma Root. El `<cmd>` va escapado, como hoy.
2. **Lectura.** Un 125 de `podman exec` es siempre de podman, y es infraestructura. Cualquier otro código distinto de 0 es un rojo del producto, incluidos 126 y 127.
3. **`PATH`.** Cada paso fija `PATH` con los directorios del sistema primero: `/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`. Después van los directorios del producto, como el `bin/` del venv del preludio de Python.

**Tests en rojo primero:**
- Unitario: un escenario que sale con 125 da un fallo del producto, no infraestructura.
- Unitario: un `instalar` que sale con 125 da "no instala".
- Unitario: un `podman exec` falso que devuelve 125 sin pasar por el envoltorio da infraestructura.
- Unitario: los argumentos de un paso fijan el `PATH` con el sistema primero.
- Con podman: un producto que deja un `curl` falso en el `bin/` del venv no hace pasar un escenario que usa `curl` y debe fallar.

### 3. Red por experimento (clase B y HIGH-3)

1. **Sonda de red.** Una vez por proceso, Root corre `podman run --rm --network pasta:--no-map-gw --entrypoint sh <imagen de la tarea> -c true`. Código 0: la red se puede usar. Otro código: no se puede. Guardá el resultado en el proceso.
2. **Repetición sin texto.** Un paso que puede necesitar red y falla sin red se repite con red, sin mirar el texto. Es un `RUN`, o un `ADD` o `COPY` con URL. El registro de la repetición y su Gate siguen como hoy.
3. **Desenlaces.**
   - Con red pasa: construye, con el hallazgo de severidad 1 "necesita red", como hoy.
   - Con red falla: rojo, "la imagen no construye".
   - La sonda dice que la red no se puede usar: queda pendiente, con el motivo "la red no se pudo configurar en este host".
4. **Fuera del camino de decisión:** `pide_red` y `red_no_configurada` ya no deciden en el build. Pueden quedar como dato de un aviso.
5. Un paso que falla y no puede necesitar red, como un `COPY` local, es rojo sin repetir.

**Tests en rojo primero:**
- Unitario: un `RUN wget` que falla sin red se repite con red, sin mirar el texto.
- Unitario: con la sonda en verde y el build con red fallando, da rojo.
- Unitario: con la sonda en rojo, queda pendiente con su motivo.
- Unitario: un `RUN echo "setup network"; false` con la sonda en verde da rojo.
- Unitario: un `COPY` local que falla da rojo sin repetir.
- Con podman: los 3 tests de la clase B quedan verdes, con su expectativa adaptada a la regla nueva.

### 4. Escenarios que piden red (HIGH-1)

1. `Escenario` (`src/oracle/tarea.rs:162`) suma `red: bool`, con `#[serde(default)]`.
2. Un escenario con `red = true` no corre. Queda pendiente con el motivo "el escenario pide red".
3. Un escenario sin `red`, que sale distinto de 0, es un fallo. No se mira su salida.
4. `pide_red` sale de `correr_escenario`.
5. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un escenario que imprime "could not resolve host" y sale con 1 da fallo.
- Unitario: un escenario con `red = true` queda pendiente sin correr: el `Exec` falso no recibe ningún `podman exec` para él.
- Guarda: un `tarea.toml` viejo sin `red` se lee con `false`.

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

Al terminar, escribí `docs/builds/informe-local-91b.md` con estas secciones:

1. **Resultado de la suite**: la base y el final sin podman, y los tests de entrega con podman, con verdes, fallas e ignorados.
2. **Causa de la clase A**: el texto real de podman y el arreglo.
3. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
