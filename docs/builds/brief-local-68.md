# Brief de la Tarea 68 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-67` (c1b2fa8). No esperes ninguna mezcla.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-68`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, `docs/interfaces.md` y sus tests.
- **No toques:** `src/oracle/calibra/regla.rs`, `src/oracle/mutacion.rs`, `src/oracle/juez.rs`, `src/oracle/requisitos.rs`, `src/oracle/entrega.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`. En `src/oracle/mod.rs`, solo un literal de test si un campo nuevo lo exige. La evaluación real no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

En Completitud y Funciona, un test nuevo que falla no detecta por sí solo. La regla pide además que baje `valor`, y los tests ruidosos mueven `valor` entre corridas. Un test ruidoso que pasa puede tapar un test real que falla.

Leé en el plan, antes de empezar:

- "Diagnóstico de los no detectados (Opus, 2026-11-05)".
- "Desempates técnicos 39 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **en Completitud y Funciona, la detección sale de los tests, no de `valor`.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 4.

1. **Test nuevo detecta.** En `motivo_de_deteccion`, para Completitud y Funciona, un test malo nuevo que no está en `ruidosos` detecta, con el motivo `test_malo_nuevo:<clave>`. Va antes que `valor`. Armá el caso así: la base tiene un test ruidoso fallando, el caso lo tiene pasando y otro test falla, con `valor` igual a la base. Rojo: hoy no detecta. Guardas:
   - un limpio donde solo cambian tests de `ruidosos` no detecta;
   - un `valor` menor sin test malo nuevo, ni total menor, ni hallazgo nuevo, sigue sin detectar;
   - Seguridad, Entrega, Estructura y las demás dimensiones no cambian.
2. **Valor en cada caso.** `.casos.jsonl` suma `valor` y `valor_base`, con `#[serde(default)]`. Rojo: hoy un caso no dice su valor.
3. **Motivo de la base.** El error pasa a "la línea base no tiene veredicto de <dimensión>: <motivo>". Rojo: hoy no dice el motivo.
4. **Nombre del test.** Renombrá `calibra_pruebas_assert_borrado_sin_test_de_muerto_dice_que_ninguno_tiene_test`, para que coincida con el motivo actual. No lleva rojo.

No corras la calibración real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-68.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
