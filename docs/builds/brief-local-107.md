# Brief de la Tarea 107 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-106` (88169de). Las Tareas 84 a 106 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-107`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, `slow-timeout` en la imagen `:4` | `imagenes/oraculo-rust/Containerfile`, `scripts/construir-imagen-oraculo.sh`, `scripts/probar-imagen-oraculo.sh` |
| 2, plazo de suite desde la base | `src/oracle/calibra.rs`, `src/oracle/mod.rs` |
| 3, reservas deterministas | `src/oracle/calibra.rs` |
| 4, señales y barrido del oráculo | `src/oracle/contenedor.rs`, `src/oracle/entrega.rs`, `src/lib.rs`, y un módulo nuevo si hace falta |
| 5, equivalente repetido | `src/oracle/tarea.rs` |

- Los puntos 2 y 3 comparten `calibra.rs`. Van en serie: primero el 3.
- Los puntos 1, 4 y 5 corren en paralelo con los demás.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`. `signal-hook` ya está.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- **No construyas la imagen.** El orquestador construye `:4` con podman después de mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Un mutante dejó en bucle un test, y la calibración colgó una hora.** El orquestador lo midió en `/home/mpera/.cache/root-wt/orquestador/medicion-109.md`. En t3 Funciona, una inyección dejó en bucle infinito a `empty_match_at_end_of_line_doesnt_panic`. El contenedor del oculto corrió 59 minutos.
- El plazo de una suite es fijo: 3600 s (`OpcionesOraculo::default`, `src/oracle/mod.rs:185`).
- El perfil `ci` de nextest (`imagenes/oraculo-rust/Containerfile:117`) no tiene `slow-timeout`. Ningún test se corta solo.
- La sonda de mutación ya usa un plazo por múltiplo de la base (Tarea 97, `src/oracle/mutacion.rs`). Las suites no.

**Un r00t que muere por señal deja su contenedor corriendo.** El `timeout` del orquestador mató a r00t, y el contenedor del oculto quedó vivo. `r00t corre` ya borra sus contenedores al recibir una señal (Tareas 84 a 88). `evaluar`, `calibrar` y `auditar` no.

**El orden de los reemplazos no es determinista.** Pregunta 10 del informe de la 106: un cerrojo cubre los reemplazos, y su orden depende de qué hilo llega primero. Con una semilla fija, la misma calibración puede dar casos distintos.

**Las entradas de equivalentes repetidas se aceptan.** Pregunta 6 del informe de la 106. Una lista curada a mano con una entrada repetida es un descuido.

## Tarea

La regla que manda: **nada se cuelga para siempre, nada queda vivo después de r00t, y una semilla fija da la misma calibración.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 1.

### 1. `slow-timeout` en la imagen `:4`

1. El perfil `ci` de `/etc/root-oraculo/nextest.toml` suma `slow-timeout = { period = "60s", terminate-after = 5 }`.
2. La imagen nueva es `root-oraculo-rust:4`. La `:3` no cambia.
3. `scripts/construir-imagen-oraculo.sh` construye `:4`. `scripts/probar-imagen-oraculo.sh` comprueba que el perfil `ci` trae `slow-timeout`.
4. Revisá la sintaxis de `slow-timeout` en la documentación de nextest 0.9.100. Anotá la fuente en el informe.

Este punto no tiene test en rojo: lo prueba el script con la imagen construida.

### 2. Plazo de suite desde la base

1. En una calibración, la base guarda cuánto tardó cada suite: contrato y oculto.
2. Cada caso corre esas suites con un plazo de 10 veces lo que tardó la base, con un mínimo de 600 s.
3. **Un test cortado por nextest** es un fallo: el junit lo trae, y el código es 100. El caso cuenta con la regla de hoy.
4. **Una suite cortada por el plazo de Root, sin junit,** es una inyección inválida en la calibración. Va a reemplazo, con el motivo "la suite pasó su plazo".
5. Fuera de la calibración, el plazo sigue en 3600 s, y una suite vencida queda sin veredicto, como hoy.

**Tests en rojo primero:**
- Unitario: con una base cuyas suites tardaron 20 s, el plazo del caso es 600 s. Con 120 s, es 1200 s.
- Unitario: un caso cuya suite vence el plazo de Root va a reemplazo, con su motivo.
- Guarda: un caso con un test cortado por nextest, con junit y código 100, cuenta como hoy.

### 3. Reservas deterministas por caso

1. En la fase en serie, cada caso con defecto recibe su inyección principal y hasta `MAX_REEMPLAZOS` reservas. Todas son distintas entre todos los casos.
2. Primero se reparten las principales, y después las reservas, por turno entre los casos.
3. Un reemplazo usa la próxima reserva de su caso. El cerrojo de reemplazos deja de hacer falta.
4. Sin reservas, el caso queda "candidatos distintos agotados".

**Tests en rojo primero:**
- Unitario: dos corridas con la misma semilla y con reemplazos dan las mismas inyecciones en el mismo caso, con 4 hilos.
- Unitario: con candidatos para una principal y una reserva por caso, ningún caso comparte una inyección.

### 4. Señales y barrido del oráculo

1. **Señales.** Al recibir SIGINT o SIGTERM, `evaluar`, `calibrar` y `auditar` quitan los contenedores que llevan el pid de este r00t en el nombre: `root-oraculo-<pid>-` (`src/oracle/contenedor.rs:423`), `root-entrega-esc-<pid>-` y `root-entrega-eval-<pid>-` (`src/oracle/entrega.rs`). Usan un plazo, y salen con 130 o 143.
2. Usá el mismo esquema que `r00t corre`: un hilo con `signal_hook::iterator::Signals`, y la limpieza dentro de `catch_unwind`.
3. **Barrido al arrancar.** Antes de lanzar un contenedor, r00t quita los contenedores `root-oraculo-*` y `root-entrega-*` cuyo pid no es un r00t vivo. Usa la regla de `/proc` del barrido de `corre`, en `src/corre/barrido.rs`, con su margen de 300 s. Si conviene compartir esa regla, movela a un módulo común y explicalo en el informe.
4. SIGKILL no se puede atrapar. Ese caso lo cubre el barrido.

**Tests en rojo primero:**
- Unitario: con un `podman` falso que anota sus llamadas, una señal durante `evaluar` quita los contenedores con el pid propio, y no otros.
- Unitario: el barrido quita un contenedor cuyo pid no existe en un `/proc` de prueba, y conserva uno de un r00t vivo.
- Guarda: los tests de señal de `corre` siguen verdes.

### 5. Equivalente repetido

1. Dos entradas iguales en `[calibracion] equivalentes` dan un error de carga del `tarea.toml`.
2. El mensaje es "equivalente repetido", con su archivo, su línea y su edición.

**Test en rojo primero:**
- Unitario: un `tarea.toml` con dos entradas iguales da el error.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

**Después de mezclar, lo hace el orquestador, no vos:** construir la imagen `:4`, pasar s2-t1 a s2-t3 a `:4`, y repetir t3 Funciona.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-107.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **`slow-timeout`**: la sintaxis y su fuente.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
