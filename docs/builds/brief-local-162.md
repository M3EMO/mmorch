# Brief de la Tarea 162 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (78).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-161`. Trae las Tareas 84 a 161 y la 127b.
- Ningún otro builder corre a la vez. **No corras mientras el usuario corre `paso0-161.sh`,** ni mientras el orquestador use podman.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-162`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el informe y la decisión de un corte de la descarga | `src/oracle/mod.rs`, y `src/oracle/informe.rs` si hace falta |
| 2, el motivo de la última falla, los guiones probados y la señal | `src/colab_calibra.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/descarga.rs`, `src/oracle/colab.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**1. Un producto que no instala sale sin veredicto y sin informe.** La verificación de la tarea b8d0db4058, con su lock real, dio un hallazgo de Entrega, severidad 3: "el producto no instala: error: failed to sync". Pero el `informe` del veredicto es nulo, y su estado es sin veredicto.

El camino es este:
- `preparar_rust`, en `src/oracle/descarga.rs`, devuelve `Preparacion::SinVeredicto` con el hallazgo.
- `evaluar_con`, en `src/oracle/mod.rs`, recibe ese valor y vuelve enseguida con `sin_veredicto(...)`.
- Ese retorno se salta `informe::informar_con`, que en el camino normal arma el informe y fija `veredicto.estado`.

La regla ya existe: `decidir`, en `src/oracle/informe.rs`, pone `Bloqueado` primero cuando hay un bloqueo, y un hallazgo de severidad 3 bloquea. El camino corto nunca llega a ella. Lo mismo pasa con los otros dos cortes de la descarga: "lock inválido" y "fuente no aceptada".

**2. Tres cosas que la 161 dejó abiertas.**
- "Colab no responde" no dice el motivo de la última falla. Cinco fallas de un guion roto se leen igual que cinco cuelgues del cliente.
- Los guiones `trabajo-N.py`, `lanzar-N.py` y `estado-N.py` solo se probaron por su texto. Un error de Python aparecería recién en Colab, después de horas.
- Falta un test de la señal que corta la espera entre consultas.

## Tarea

La regla que manda: **todo veredicto pasa por la regla de decisión, y un guion que va a Colab se prueba antes de subir.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El informe y la decisión de un corte de la descarga

1. **El camino de `Preparacion::SinVeredicto` arma su informe** con `informe::informar_con`, igual que el camino normal, y fija `veredicto.estado` con `informe::estado_de`.
2. **Sin redactor.** El informe de un corte es determinista. No llama a ninguna API.
3. **Las nueve dimensiones siguen sin veredicto,** con el motivo de hoy.
4. **La decisión sale de `decidir`, sin regla nueva.** Con el hallazgo de severidad 3, se espera `Bloqueado`.
5. **Si `bloqueos` no cuenta ese hallazgo,** por ejemplo porque depende de una rúbrica, no agregues una regla. Detenete y anotalo en el informe, con la función y el motivo.
6. **El informe nombra el hallazgo** y dice que las suites no corrieron.
7. **Vale para los tres cortes:** "no instala", "lock inválido" y "fuente no aceptada".

**Tests en rojo primero:**
- Unitario: una evaluación con un `Preparacion::SinVeredicto` de "no instala" da un veredicto con informe, decisión `Bloqueado` y estado rojo. Hoy da informe nulo y estado sin veredicto.
- Unitario: lo mismo con "lock inválido" y con "fuente no aceptada".
- Guarda: las nueve dimensiones siguen sin veredicto.
- Guarda: el camino normal no cambia.

### 2. El motivo de la última falla, los guiones probados y la señal

1. **"Colab no responde" suma el motivo corto de la última falla:** por ejemplo "plazo de 180 s", "código 1" o "sin marcador". Nunca suma la salida entera.
2. **Un test corre los tres guiones con `python3`,** en un temporal del test, con un comando falso que duerme 2 segundos y sale con código 3:
   - `lanzar-N.py` imprime `ROOT_LANZADO` y vuelve enseguida;
   - `estado-N.py` da `corriendo` mientras el comando vive;
   - `estado-N.py` da `fin` con código 3, inicio y fin, cuando termina;
   - con el pid muerto y sin estado, `estado-N.py` da `murio`, con el final del log.
3. **Si el test no puede correr `python3`,** que quede ignorado con su motivo, como en la 149.
4. **Un test de la señal:** una señal durante la espera entre consultas corta la espera, y Root libera el runtime como hoy.
5. **Las rutas de los guiones en el test** van dentro del temporal. El test no usa `/content`.

**Tests en rojo primero:**
- Unitario: con 5 consultas que fallan por plazo, el mensaje dice "plazo". Hoy no lo dice.
- Integración: los tres guiones corren con `python3` y dan las respuestas de arriba. Hoy ningún test los corre.
- Unitario: una señal durante la espera corta la espera.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 162.
- La verificación otra vez de b8d0db4058 con su lock real. Debe dar `Bloqueado`, con informe.

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

Al terminar, escribí `docs/builds/informe-local-162.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La decisión de un corte**: qué da `decidir` para cada uno de los tres cortes, y por qué regla.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
