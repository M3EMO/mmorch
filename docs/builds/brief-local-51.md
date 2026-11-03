# Brief de la Tarea 51 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Otros builders trabajan en paralelo en otras zonas de archivos.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-51`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/descarga.rs`, todo `src/oracle/descarga/`, sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/calibra*`, `src/oracle/mutacion.rs`, `src/oracle/mod.rs`, `src/oracle/entrega.rs`, `src/oracle/contenedor.rs`, `src/corre/`, `src/observe/`, `src/track/` y `src/config.rs`.
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
- La fila de la Tarea 51 en "Tareas 51 a 55: arreglos del code review, por zona de archivos".

## Tarea

Hacé un commit por hallazgo, o por grupo chico de hallazgos de la misma función. Cada commit nombra el id del hallazgo, por ejemplo `H3`.

1. **H3, gramática estricta de pip.** Cada especificador de `requirements.txt` y de `[build-system].requires` pasa por una gramática PEP 508 estricta. Un espacio seguido de `-`, un `\n` o un `\r` rechaza la línea con su número. Test: `foo==1.0 --no-binary :all:` se rechaza, en los dos archivos.
2. **H5, dependencias de runtime.** Root lee `Requires-Dist` del `PKG-INFO` de cada sdist y `[project].dependencies` de cada clon, sin ejecutar nada, y suma esos requisitos al índice de wheels. Si los metadatos son dinámicos, la descarga da sin veredicto con el motivo "dependencias de runtime no declaradas en forma estática". Test: un sdist que declara `requests` deja a `requests` en el índice.
3. **M15 y M22, sin seguir symlinks.** `textos_de` del juez de config y la lectura del `pyproject.toml` de un clon leen solo archivos regulares dentro del repo, con un tope de tamaño. Test: un symlink a un archivo de afuera no se lee.
4. **L10, redactar antes de truncar.** Root redacta los secretos de un texto antes de cortarlo a 300 caracteres. Test: un token que cae en el corte no aparece.
5. **L13, versión con comodín.** `==1.*` se trata como un comodín de versión, no como la versión `1.`. Test: no da un falso "no instala".
6. **L14, error de lectura.** Un error al leer un archivo de requisitos da un error de infraestructura, no una lista vacía.
7. **Acotar `correr_podman` de la descarga** (pregunta 2 del informe 52). La salida que lee la descarga conserva solo los últimos `COLA` bytes, como hace `leer_en_hilo` del contenedor del oráculo.
8. **C2, solo si el orquestador te pasa la respuesta del usuario a P62.** Seguí la opción que elija el usuario, tal como la describe P62 en el plan. Sin esa respuesta, no hagas C2 y anotalo en el informe.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-51.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
