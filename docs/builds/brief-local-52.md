# Brief de la Tarea 52 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Otros builders trabajan en paralelo en otras zonas de archivos.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-52`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/entrega.rs`, `src/oracle/contenedor.rs`, sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/calibra*`, `src/oracle/mutacion.rs`, `src/oracle/mod.rs`, `src/oracle/descarga*`, `src/corre/`, `src/observe/`, `src/track/` y `src/config.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Un `/ecc:code-review` del hito encontró 2 hallazgos críticos, 5 altos y 22 medios. Opus verificó los críticos y los altos en el código. El informe es `~/.cache/root-wt/orquestador/review-47-informe.md`. Leé las filas de tus hallazgos, con su ubicación, su evidencia y su escenario de falla.

Leé en el plan, antes de empezar:

- "Code review del hito (Tarea 47, adelantado, 2026-11-03)", con su tabla de verificación.
- "Desempates técnicos 27 (Opus, 2026-11-03)".
- La fila de la Tarea 52 en "Tareas 51 a 55: arreglos del code review, por zona de archivos".

## Tarea

Hacé un commit por hallazgo, o por grupo chico de hallazgos de la misma función. Cada commit nombra el id del hallazgo, por ejemplo `H3`.

1. **H4, paso de buildah por texto.** Root ubica la instrucción que falló por el texto del paso que imprime buildah, no por su número. Si no la encuentra, la construcción queda sin veredicto, no roja. Tests con podman: un Dockerfile de dos etapas con un `RUN` de red que falla en la segunda queda `Pendiente`, y un `ARG` antes del primer `FROM` no corre el índice.
2. **M19, red de verdad.** Un escenario queda pendiente por red solo con errores de DNS o de red inalcanzable. `connection refused` y `failed to connect` cuentan como falla del producto.
3. **M20, arranque del contenedor de entrega.** El contenedor de entrega escribe un marcador al arrancar. Sin ese marcador, la entrega es infraestructura y da sin veredicto, no "no instala".
4. **M21, Python en la entrega.** La entrega de una tarea Python aplica `PRELUDIO_PYTHON`. Si no puede, da sin veredicto.
5. **M7, salida con tope.** `leer_en_hilo` conserva solo los últimos `COLA` bytes y descarta el resto. Test: un comando que imprime 100 MB no sube la memoria por encima de un tope.
6. **M1, endurecer el contenedor del oráculo.** El `podman run` del oráculo suma `--cap-drop=all`, `--security-opt=no-new-privileges`, `--pids-limit`, `--memory` y `--pull=never`. Medí con podman que las suites de memora y brasa de fixture siguen corriendo. Si una opción rompe algo, anotalo y dejala afuera.
7. **L2, limpieza con plazo.** `podman rm -f` corre con un plazo, también en la rama de error de `esperar`.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-52.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
