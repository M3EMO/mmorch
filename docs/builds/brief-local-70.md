# Brief de la Tarea 70 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-69` (3951cf2).
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-70`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/` y sus tests.
- **No toques:** `src/oracle/`, `src/route/`, `src/observe/`, `src/proveedores/`, `src/config.rs` y `src/tipos.rs`. No imprimas tokens.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La validez automática corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Una corrida que muere por una señal deja la copia de la credencial en disco: hay una real, con `refreshToken`. Un builder matado con 137 pierde su transcript. Un transcript puede guardar el token sin redactar.

Leé en el plan, antes de empezar:

- "Verificación de la revisión de cierre (Opus, sobre laptop/tarea-69)".
- "Desempates técnicos 41 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un token no queda en disco fuera de su lugar, y un transcript nunca se pierde.** Hacé un commit por punto. Cada punto empieza con su test en rojo. Ningún mensaje ni test imprime un token real.

1. **Barrido.** Al empezar `r00t corre`, antes de armar nada, Root revisa cada carpeta de corridas. Si `claude-estado/.credentials.json` existe y el pid del id de la corrida ya no vive, la borra con `src/borrado.rs` y deja un `Gate`, sin el contenido. Un id sin pid legible también se barre. Rojo:
   - una corrida muerta conserva hoy su copia, y debe perderla;
   - la copia de un pid vivo no se toca;
   - un enlace se borra como enlace.
2. **Podman con 137.** Un 137 no descarta `stdout`. Pasa a `ErrorCorre::Cortado`, con el motivo "el contenedor fue matado por SIGKILL". Así guarda el transcript y sigue como vuelta. 125, 126 y 127 no cambian. Rojo: hoy un 137 da `Ejecutor` y pierde `stdout`.
3. **Transcript redactado.** Antes de escribir un transcript, Root reemplaza el `accessToken` de la copia vigente por `[REDACTADO]`. Vale también en un corte. La copia se lee antes de borrarla. Sin token legible, escribe el transcript igual y deja un aviso. Rojo: un `tool_result` con el token real de la copia de prueba hoy queda en el archivo. Armá el transcript con la forma real de una línea `user` con `tool_result`.

No corras la corrida real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-70.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
