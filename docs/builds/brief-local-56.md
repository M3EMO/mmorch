# Brief de la Tarea 56 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de las Tareas 52 y 53**. El orquestador te dice el commit. Otros builders trabajan en paralelo en otras zonas: la 51 en `src/oracle/descarga*` y la 54 en el núcleo de `src/oracle/`.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-56`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/entrega.rs`, `src/config.rs`, `tests/proveedores.rs`, el stream de `src/proveedores/` solo si hace falta inyectar el reloj, sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/calibra*`, `src/oracle/juez.rs`, `src/oracle/evidencia.rs`, `src/oracle/descarga*`, `src/oracle/contenedor.rs`, `src/corre/`, `src/observe/` y `src/track/`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Los informes de las Tareas 52 y 53 dejaron preguntas, y el orquestador encontró un test frágil. Leé `docs/builds/2026-11-03-build-local-52.md` y `docs/builds/2026-11-03-build-local-53.md`, en sus ramas o mezclados.

Leé en el plan, antes de empezar:

- "Desempates técnicos 28 (Opus, 2026-11-03): informes de las Tareas 48, 50, 52 y 53".
- "Tarea 56: Entrega sin verde falso, config estricta y test de stream con reloj".

## Tarea

Hacé un commit por parte. Cada commit nombra la pregunta o el hallazgo, por ejemplo `52-P1`.

1. **Entrega sin verde falso** (pregunta 1 del informe 52 y hallazgo 6). Una construcción `Pendiente`, o una imagen base que no se descargó, deja su ítem de Entrega en `faltantes` con su motivo. Entrega toma su valor de los ítems medidos. Si no midió ninguno, queda sin veredicto. Tests: un Dockerfile con un paso fallido por red, y sin otros hallazgos, no da Entrega 1,0; una entrega sin ningún ítem medido da sin veredicto.
2. **Config estricta** (pregunta 1 del informe 53). `Ruteo`, `ModeloApi`, `Modelos` y `ConfigProveedores` llevan `deny_unknown_fields`. Test: `timeout_juz_s = 10` en `[proveedores]` sale con código 2. Revisá que `docs/root.toml.example` siga cargando.
3. **Balance validado** (pregunta 5 del informe 53). `tope_usd` es finito y no negativo, `prefiere_api_sobre` está entre 0 y 1, y `bloque_horas` es mayor que 0. Si no, la carga sale con código 2.
4. **Test de stream con reloj** (hallazgo 1). `proveedores_stream_con_un_trozo_cada_60_s_no_corta_por_inactividad` usa un reloj inyectado, sin plazos reales. Corrélo 20 veces seguidas con la máquina cargada por otro `cargo build`, sin fallas.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-56.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
