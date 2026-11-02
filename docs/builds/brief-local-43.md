# Brief de la Tarea 43 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e`, que trae este archivo.
- Trabajá en una rama local nueva, por ejemplo `laptop/tarea-43`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La tarea no necesita podman, claves de API, Colab ni el `ROOT_HOME` real.
- Corré la suite con `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`. Así no toma el lock de podman y no choca con el Validate en curso.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- El hook pre-commit corre la suite completa con podman y toma el lock. Si hay un Validate en curso, el hook espera su turno.

## Tarea

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-02: prueba de Haiku, Tarea 42 y su mezcla".
- "Desempates técnicos 21".
- "Tarea 43: Inactividad de Anthropic, módulos sin perfil y `bash` en los tests".

La tarea tiene tres partes. Hacé un commit por parte.

1. **Inactividad de Anthropic.** La Tarea 41 (commit b790507) quitó el corte de inactividad para Anthropic con razonamiento. Una medición real mostró que Anthropic manda un `ping` cada 30 segundos, y Root corta por bytes. Volvé a Anthropic al mismo plazo de inactividad de 120 segundos que DeepSeek. Quitá la excepción y los tests que la fijaban.
2. **Módulos sin perfil.** Si el lenguaje de la tarea no tiene perfil, Root no conoce los módulos del repo. En ese caso, `faltante` (`src/oracle/informe.rs`) no filtra la lista `modulos`. Nombra los tests que no pasan, como antes de la decisión 56. Con perfil, la regla de la decisión 56 no cambia.
3. **`bash` en los tests.** `tools/test-seguro.sh` usa `BASH_SOURCE`, así que es un script de bash. `tests/test_seguro.rs` lo lanza con `bash <ruta>`. Los ejecutables falsos, que son POSIX, siguen con `sh <ruta>`.

Los criterios de `cargo test` están en la sección "Validate" de la Tarea 43.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-43.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte y commit.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
