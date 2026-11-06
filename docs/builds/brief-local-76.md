# Brief de la Tarea 76 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-75` (52dbb3b).
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-76`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/auditar.rs` y sus tests, `src/cli.rs` y `src/lib.rs` (solo este subcomando) y `docs/interfaces.md`.
- **No toques:** `PROMPT_AUDITORIA`, `src/oracle/juez.rs`, `src/oracle/validez.rs`, `src/oracle/calibra/`, `src/corre/`, `src/route/`, `src/proveedores/`, `$ROOT_HOME/` y `~/.cache/root-wt/`. No abras archivos de defectos fuera del fixture.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La corrida real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

`--solo-medir` no pide reporte de gitleaks, así que una cita puede copiar un secreto al JSON de salida. Con repos minados, ese JSON se comparte. Además, con `--defectos` y ningún archivo leído, `medicion` queda en `null` sin decir por qué.

## Tarea

Hacé un commit por punto. Cada punto empieza con su test en rojo. Los tests usan un fixture, y nunca una API.

1. **`--gitleaks` siempre.** `--solo-medir` también lo exige. Sin él, el subcomando sale con 2. Rojo: hoy `--solo-medir` sin reporte sale con 0.
2. **Línea de secreto.** En `--solo-medir`, un candidato cuya cita cae en una línea del reporte va a `descartados`, sin copiar su texto. Rojo: con un reporte que cita la línea 3, y un candidato sobre esa línea, hoy su cita entra en `hallazgos`.
3. **Aviso.** Con `--defectos` y ningún archivo leído, `avisos` suma "ningún archivo leído: no hay medición". Rojo: hoy no hay aviso.

**Guarda.** `hash_prompt` no cambia.

No corras la pasada real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-76.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
