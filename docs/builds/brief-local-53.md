# Brief de la Tarea 53 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Otros builders trabajan en paralelo en otras zonas de archivos.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-53`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/observe/`, `src/track/`, `src/aprende.rs`, `src/config.rs`, `src/mcp.rs`, `src/lib.rs` (solo `observa claude`), `src/oracle/historial.rs`, sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/calibra*`, `src/oracle/mutacion.rs`, `src/oracle/mod.rs`, `src/oracle/entrega.rs`, `src/oracle/contenedor.rs`, `src/oracle/descarga*` y `src/corre/`.
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
- La fila de la Tarea 53 en "Tareas 51 a 55: arreglos del code review, por zona de archivos".

## Tarea

Hacé un commit por hallazgo, o por grupo chico de hallazgos de la misma función. Cada commit nombra el id del hallazgo, por ejemplo `H3`.

1. **M3, infraestructura no es fracaso.** Las filas `SinVeredicto` y `SinDecision` no cuentan en `n` ni en los éxitos de Wilson, en `aprende` y en `track`. Se cuentan aparte. Test: una fila sin veredicto no baja la cota de un modelo.
2. **M9, línea truncada.** Bajo el lock, `jsonl` escribe un `\n` antes de agregar si el archivo no termina en `\n`. Test: después de una línea cortada, el lector lee todas las líneas siguientes.
3. **M10, filas con lock.** `filas_de` lista los archivos con `jsonl::con_lock_compartido`, como `observe::uso`.
4. **M11 y L5, config estricta.** `Config`, `Balance` y `TopeBloque` llevan `deny_unknown_fields`. `umbral_wilson` debe ser finito y estar entre 0 y 1. Test: `tope_usdd = 5` en `[balance.api]` sale con código 2.
5. **L4, `observa claude` idempotente.** Registrar dos veces la misma salida de `claude -p` no duplica el costo. La clave es el `session_id` de la sesión.
6. **L6, un Gate por tope lleno.** Dos `observa uso` a la vez dejan un solo Gate `tope-lleno`.
7. **L1, marca de `plan-aprobado`.** `root_registrar` con `plan-aprobado` registra `extra.origen = "mcp"`. Así un plan aprobado por MCP no parece una aprobación humana tecleada.
8. **L12, historial con lock.** `historial.rs` agrega con lock.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-53.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
