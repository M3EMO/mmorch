# Brief de la Tarea 45 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Esa punta ya incluye la Tarea 44 (mezcla 25084f1).
- Trabajá en una rama local nueva, por ejemplo `laptop/tarea-45`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`. Así no toma el lock de podman.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- El hook pre-commit corre la suite completa con podman. Mientras corran evaluaciones con podman, no commitees: el Validate no toma el lock. Esperá la señal del orquestador.

## Por qué

La Tarea 44 hizo que el juez responda todos los ítems pedidos, con un reintento. Su build dejó cuatro huecos: el motivo de un ítem que falla dos veces pierde su causa, un nivel inválido no se reintenta, la calibración no puede contar los reintentos, y falta un test del tope con `JuezProveedor`.

## Tarea

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-02: brasa del Validate, Tarea 44 y su mezcla".
- "Desempates técnicos 23 (Opus, 2026-11-02): preguntas de la Tarea 44".
- "Tarea 45: Motivos del reintento, nivel inválido y marca de reintento".
- El informe `docs/builds/2026-11-02-build-local-44.md`.

La tarea tiene cuatro partes. Hacé un commit por parte.

1. **Motivos con causa.** Un ítem que sigue sin nivel después del reintento conserva su causa en `faltantes`. Los motivos son tres: "la evidencia no pasó el gate tras un reintento", "el juez no dio un nivel válido tras un reintento" y "el juez no respondió tras un reintento".
2. **Nivel inválido.** Un nivel mal tipado, como "alto", o fuera de 0 a 3, rompe el contrato y va al reintento.
3. **Marca de reintento.** El evento de la llamada de reintento lleva `extra.reintento_juez = true`.
4. **Test del tope.** Un test de integración arma un `ROOT_HOME` de prueba, una clave falsa y un servidor HTTP falso en `127.0.0.1`. Con el tope de API lleno, `JuezProveedor` no envía el reintento. El test nunca llega a una API real.

Los criterios de `cargo test` están en la sección "Validate" de la Tarea 45.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-45.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte y commit.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
