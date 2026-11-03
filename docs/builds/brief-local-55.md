# Brief de la Tarea 55 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de la Tarea 50 y de la Tarea 54**. El orquestador te dice el commit.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-55`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/`, `comando_git` y `commit_de` de `src/oracle/mod.rs`, un módulo nuevo para git seguro si hace falta, sus tests y `docs/interfaces.md`.
- **No toques:** el resto de `src/oracle/`, `src/observe/`, `src/track/` y `src/config.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Un `/ecc:code-review` del hito encontró 2 hallazgos críticos, 5 altos y 22 medios. Opus verificó los críticos y los altos en el código. El informe es `~/.cache/root-wt/orquestador/review-47-informe.md`. Leé las filas de tus hallazgos, con su ubicación, su evidencia y su escenario de falla.

Leé en el plan, antes de empezar, también "Desempates técnicos 28 (Opus, 2026-11-03): informes de las Tareas 48, 50, 52 y 53". Y además:

- "Code review del hito (Tarea 47, adelantado, 2026-11-03)", con su tabla de verificación.
- "Desempates técnicos 27 (Opus, 2026-11-03)".
- La fila de la Tarea 55 en "Tareas 51 a 55: arreglos del code review, por zona de archivos".

## Tarea

Hacé un commit por hallazgo, o por grupo chico de hallazgos de la misma función. Cada commit nombra el id del hallazgo, por ejemplo `H3`.

1. **C1, git del host sobre un repo limpio.** Al terminar cada rol, Root copia el árbol de trabajo del builder, sin `.git` ni enlaces, a un repo nuevo del host. Hace el commit ahí. El host nunca corre git sobre el `.git` que escribió el builder.
2. **C1, git endurecido.** Todo git sobre un repo que vino del builder corre con `-c core.fsmonitor=false -c core.hooksPath=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null` y sin filtros. Vale para `comando_git` y `commit_de`. Test: un repo con `core.fsmonitor` que escribe un archivo marca no lo escribe al pasar por Root.
3. **M2, fallos del runner.** Los códigos 126, 127 y 137 de podman, un token vencido y un rol sin `result` exitoso dan sin veredicto, sin evaluar y sin fila roja.
4. **Subagentes que no mueren** (hallazgos 2 y 3). `env_del_builder` fija `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS` igual al plazo del rol en milisegundos, no 0. Test: el entorno del builder trae ese valor.
5. **Costo de un rol cortado** (hallazgo 4). Si el runner corta un rol por tiempo, suma el `usage` de los mensajes del transcript `stream-json` y registra ese costo con `extra.costo_estimado = true`. Test: un transcript cortado de fixture deja un evento con costo mayor que 0.
6. **`registrar_sesion` sin duplicar** (pregunta 3 del informe 53). Registrar dos veces la misma sesión no duplica el costo. La clave es el `session_id`.
7. **M1, endurecer el contenedor del builder.** El `podman run` del builder suma `--cap-drop=all`, `--security-opt=no-new-privileges`, `--pids-limit` y `--memory`. Medí con la imagen real que `claude` todavía corre. Si una opción rompe `claude`, anotala y dejala afuera.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-55.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
