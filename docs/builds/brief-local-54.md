# Brief de la Tarea 54 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de las Tareas 48 y 49, 52 y 53**. El orquestador te dice el commit. No arranques antes: esas tareas cambian estos mismos archivos.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-54`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/mod.rs` (salvo `comando_git` y `commit_de`), `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/evidencia.rs`, `src/oracle/juez.rs`, `src/oracle/tarea.rs`, `src/oracle/pendientes.rs`, sus tests, un plugin nuevo de pytest bajo `imagenes/` o `src/oracle/`, y `docs/interfaces.md`.
- **No toques:** `comando_git` y `commit_de` de `src/oracle/mod.rs` (son de la Tarea 55), `src/corre/`, `src/oracle/descarga*`, `src/oracle/entrega.rs`, `src/oracle/contenedor.rs`, `src/observe/`, `src/track/` y `src/config.rs`.
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
- La fila de la Tarea 54 en "Tareas 51 a 55: arreglos del code review, por zona de archivos".

## Tarea

Hacé un commit por hallazgo, o por grupo chico de hallazgos de la misma función. Cada commit nombra el id del hallazgo, por ejemplo `H3`.

1. **H1, tests ocultos borrados después de la colección.** Root corre la suite oculta con un plugin propio de pytest. Después de la colección, el plugin borra los `*.py` de la copia de los tests ocultos. Los datos, como `oculto/muestras/`, quedan. El junit se escribe con un nombre aleatorio por corrida. Test con podman: un producto de fixture que intenta leer un test oculto mientras corre no lo encuentra, y la suite da el mismo resultado que antes. Documentá en `docs/interfaces.md` el límite que queda: un producto que corre dentro del proceso de pytest puede reescribir el junit.
2. **H2, identidad de versión de Colab.** `pendientes.rs` usa `historial::clave_de_version` como identidad, no `commit_de`. Test: un cambio sin commit cambia la identidad.
3. **M4, `ErrorTragado` compilable.** El defecto exige una sentencia completa, con paréntesis balanceados. Test: una línea de continuación `)?;` no se elige.
4. **M5, muestra mínima.** Una dimensión queda calibrada solo con al menos 5 defectos aplicables y 5 casos limpios. Test: con `--n 2`, la dimensión no queda calibrada.
5. **M6, suites que no son pytest.** Una suite no-pytest con 0 tests da sin veredicto. Una salida distinta de 0 con un junit verde da sin veredicto.
6. **M8, cita mínima.** El gate de evidencia exige al menos 8 caracteres alfanuméricos en la cita. Test: una cita `}` no pasa.
7. **M18, nombre de la tarea.** `Tarea.nombre` del TOML debe coincidir con su carpeta. Si no coincide, es un error de tarea con código 2.
8. **L7, prompt del juez.** El prompt dice que el juez no juzga estilo, salvo lo que pide el ancla de un ítem. No cambies las rúbricas.
9. **Tope en cada reemplazo** (pregunta 8 del informe 48). La calibración revisa el tope antes de cada reemplazo de un caso, no solo antes del caso.
10. **Reservas contadas una vez** (pregunta 15 del informe 48). El coordinador de casos en paralelo no suma su propio peor caso si la reserva de ese caso ya está en `eventos.jsonl`. Test: con 4 casos en vuelo, el gasto contado es la suma de 4 reservas, no de 8.
11. **Lock de la carpeta temporal** (pregunta 17 del informe 48). Cada evaluación toma un lock sobre su carpeta `evaluar-*`, y `limpiar_temporales` salta las carpetas con lock. Test: una carpeta con lock de más de 24 horas no se borra.
12. **Memo por tarea** (pregunta 18 del informe 48). `MemoSondas` filtra por tarea.
13. **Archivo de casos en el error del tope** (pregunta 20 del informe 48). El mensaje nombra el archivo `.casos.jsonl`.

Leé también "Desempates técnicos 28 (Opus, 2026-11-03): informes de las Tareas 48, 50, 52 y 53". El turno global de contenedor ya vive en `correr_podman` de `src/oracle/contenedor.rs`: no lo muevas.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-54.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
