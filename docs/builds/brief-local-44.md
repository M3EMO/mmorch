# Brief de la Tarea 44 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Si la Tarea 43 todavía no está mezclada, arrancá desde `laptop/tarea-43` (d084fbc).
- Trabajá en una rama local nueva, por ejemplo `laptop/tarea-44`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La tarea no necesita podman, claves de API, Colab ni el `ROOT_HOME` real. Todos los tests usan un juez falso.
- Corré la suite con `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`. Así no toma el lock de podman y no choca con el Validate en curso.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- El hook pre-commit corre la suite completa con podman. Si hay un Validate en curso, no commitees: el Validate no toma el lock. Esperá la señal del orquestador.

## Por qué

El juez de diseño y arquitectura (`src/oracle/juez.rs`) omite ítems distintos en cada corrida. Su prompt dice: "Sin evidencia para un ítem, no lo incluyas en items." Omitir no le cuesta nada al juez. En 4 corridas de memora, faltaron entre 1 y 6 ítems de 10, y Arquitectura y Diseño cambiaron de valor con el mismo commit. La calibración mide al juez, así que este contrato entra antes.

## Tarea

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-02: Tarea 43, Validate de memora y contrato del juez".
- "Desempates técnicos 22", parte "Contrato del juez (decisión 57)".
- "Tarea 44: Contrato del juez con ítems obligatorios y un reintento".
- El desempate 3 de "Desempates técnicos 4": un 0 sin evidencia es un rojo falso.

La tarea tiene tres partes. Hacé un commit por parte.

1. **Ítems obligatorios.** Root pasa al juez la lista exacta de ids que debe responder, en orden fijo. `PROMPT_SISTEMA` cambia: el juez responde cada id una sola vez. Cada respuesta trae un nivel de 0 a 3 con su evidencia, o `"nivel": null` con `"motivo": "sin_evidencia"`. Quitá la frase "Sin evidencia para un ítem, no lo incluyas en items".
2. **Lectura de la respuesta.**
   - Un id con `"motivo": "sin_evidencia"` va a `faltantes` con el motivo "el juez no encontró evidencia". No vale 0, y no hace reintento.
   - Un id pedido que falta, que se repite, o que no trae nivel ni `sin_evidencia`, rompe el contrato y va al reintento.
   - Un id que no se pidió se ignora y deja un aviso.
3. **Un reintento.**
   - Root hace un solo reintento por juicio, solo con los ids que rompieron el contrato y los que no pasaron el gate de evidencia.
   - El prompt del reintento nombra, para cada id del gate, la cita que no se encontró y su `archivo:línea`.
   - Si el reintento tampoco responde un id, ese id va a `faltantes` con el motivo "el juez no respondió tras un reintento".
   - El reintento pasa por el chequeo previo de tope, como toda llamada. Usa el mismo `max_tokens` del juez.

Los criterios de `cargo test` están en la sección "Validate" de la Tarea 44.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-44.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte y commit.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
