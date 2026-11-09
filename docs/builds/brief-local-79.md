# Brief de la Tarea 79 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (6403196).
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-79`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `imagenes/oraculo-rust/Containerfile`, `scripts/construir-imagen-oraculo.sh`, `scripts/probar-imagen-oraculo.sh`, `src/ficha.rs` (solo `borrador_tarea`), `tests/` y `docs/interfaces.md`.
- **No toques:** el resto de `src/`, y `$ROOT_HOME/`. La etiqueta `:2` de la imagen sigue existiendo.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La minería y V0 corren mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Las tareas del dataset son de Rust. `cargo test` no da junit, y la imagen no trae `cargo nextest`. `borrador_tarea` tiene comandos de pytest fijos. Sin esto, S2 no puede evaluar una tarea minada.

**Espera P91.** No instales nada hasta que el orquestador confirme esa respuesta.

## Tarea

La regla que manda: **una tarea de Rust da resultados por test, igual que pytest.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 3.

1. **Imagen `:3`.** La imagen suma `cargo-nextest`, con versión fija y su fuente en un comentario, como las demás herramientas. La versión `:2` no cambia. `probar-imagen-oraculo.sh` comprueba `cargo nextest --version`.
2. **`borrador_tarea` con runner.** Acepta `runner = "nextest"`. Con él, `contrato.cmd` y `oculto.cmd` corren `cargo nextest run --offline --profile ci` y copian `target/nextest/ci/junit.xml` a `/salida/<xml>`. Sin `runner`, sale como hoy. Rojo: hoy el borrador siempre usa pytest.
3. **Guarda con la herramienta real.** Un crate de fixture con un test verde y uno rojo se evalúa con la imagen `:3`. El veredicto trae 1 y 1. La corre el orquestador, con podman.

**Guarda.** Una tarea con pytest da el mismo veredicto.

No corras la validez, la corrida ni la pasada reales. Las corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-79.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
