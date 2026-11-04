# Brief de la Tarea 58 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de la Tarea 54**. El orquestador te dice el commit. Otros builders trabajan en paralelo: la 55 en `src/corre/` y en `comando_git` y `commit_de` de `src/oracle/mod.rs`, y la 57 en la descarga.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-58`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/mod.rs` (salvo `comando_git` y `commit_de`), `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/memo.rs`, `src/oracle/entrega.rs`, y en `src/proveedores/` y `src/observe/` solo lo necesario para etiquetar la reserva con su caso. También sus tests y `docs/interfaces.md`.
- **No toques:** `comando_git` y `commit_de` de `src/oracle/mod.rs`, `src/corre/`, `src/oracle/descarga*`, `src/oracle/contenedor.rs`, `src/config.rs` y `src/lib.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Si la calibración real corre al mismo tiempo, exportá `ROOT_SIN_PODMAN=1`. Así tus tests y tu hook no cargan la CPU con podman. La suite con podman corre al mezclar.

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Los informes de las Tareas 54 y 56 dejaron puntos sin hacer, porque sus archivos estaban fuera de la lista. Leé `docs/builds/2026-11-03-build-local-54.md` y `docs/builds/2026-11-03-build-local-56.md`.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04: mezclas hasta 8b417db y Tarea 54 terminada".
- "Desempates técnicos 29 (Opus, 2026-11-04)".
- La sección de tu tarea en el plan.

## Tarea

Hacé un commit por punto. Cada commit nombra el informe y la pregunta, por ejemplo `51-P4`.

1. **Reservas contadas una vez (54-P1).** Cada reserva del tope lleva el id del caso que la hizo. El coordinador de casos en paralelo no suma su propio peor caso para un caso que ya tiene reserva. Test: con 4 casos en vuelo y sus 4 reservas escritas, el gasto contado es la suma de 4 reservas, no de 8.
2. **Memo por tarea (54-P2).** `MemoSondas::mutantes_muertos` (`src/oracle/memo.rs`) filtra por tarea.
3. **Test frágil (hallazgo 8).** Ante errores de varios casos, el coordinador propaga el del caso con el índice más bajo, no el primero en el tiempo. Corré `calibra_un_caso_con_error_corta_y_se_propaga_el_primero` 20 veces seguidas con carga, sin fallas.
4. **Entrega mide solo sus ítems (56-P1).** Los requisitos son de Completitud y no cuentan como medidos en Entrega. Ajustá los dos tests de `src/oracle/mod.rs` que fijan el caso: `oraculo_sin_dockerfile_no_llama_a_podman_y_entrega_da_veredicto` y `oraculo_con_sondas_sanas_solo_diseno_y_arquitectura_quedan_sin_veredicto`.
5. **`faltantes` por ítem en Entrega (56-P2).** `ResultadoEntrega` suma `faltantes`, con `#[serde(default)]`. Un build pendiente lleva su ítem a `Puntaje.faltantes`, con su motivo.
6. **Tope de salida en `entrega.rs`.** Las lecturas de salida de `entrega.rs` conservan solo los últimos `COLA` bytes.
7. **Ruta de la suite oculta, solo si el orquestador te lo pide.** La evaluación de humo de memora y brasa puede mostrar que la ruta nueva `/salida/.oculto` rompe los `tarea.toml` reales, que llaman a pytest con `/oraculo/oculto` y `--junitxml=/salida/oculto.xml`. Si el orquestador te pasa ese resultado, hacé que la suite oculta corra desde la copia aunque el `cmd` nombre la ruta literal, y que el junit se lea donde pytest lo escribe.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-58.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
