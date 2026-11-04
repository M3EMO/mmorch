# Brief de la Tarea 60 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (10ad8d3 o posterior). La Tarea 58 trabaja en paralelo en el núcleo de `src/oracle/`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-60`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `tests/colab.rs` y `src/oracle/colab.rs`.
- **No toques:** todo lo demás, en especial `src/oracle/mod.rs`, `src/oracle/calibra*`, `src/oracle/entrega.rs`, `src/corre/` y `src/proveedores/`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.


- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Dos tests de `tests/colab.rs` fallaron con la máquina cargada: `colab_tope_minutos_001_deja_gate_tope_de_duracion` y `colab_sigterm_durante_la_corrida_libera_y_sale_3`. El segundo falló con "PID numérico: ParseIntError { kind: Empty }" en `tests/colab.rs:554`.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04 (tarde): 55, 57 y 59 mezcladas, corrida real de memora en curso".
- "Desempates técnicos 31 (Opus, 2026-11-04)".
- La sección de tu tarea en el plan.

## Tarea

Hacé un commit por punto.

1. **Tope de minutos.** `colab_tope_minutos_001_deja_gate_tope_de_duracion` usa un plazo inyectado, sin esperas reales.
2. **SIGTERM.** `colab_sigterm_durante_la_corrida_libera_y_sale_3` espera a que el falso escriba su PID completo antes de leerlo. Si hace falta, el falso escribe el PID en un archivo temporal y lo renombra, para que nunca se lea a medias.
3. **Prueba con carga.** Corré cada test 20 veces seguidas con otro `cargo build` en paralelo. Ninguno falla.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-60.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
