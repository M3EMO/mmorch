# Brief de la Tarea 71 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `e665a5f`, la punta de `claude/root-e7813e`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-71`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/entrega.rs`, `src/oracle/contenedor.rs`, `src/oracle/perfiles/herramientas.rs`, `src/oracle/junit.rs`, `src/oracle/informe.rs`, `src/oracle/juez.rs`, y en `src/oracle/mod.rs` solo la lectura de `/salida`. En `src/oracle/calibra.rs`, solo el test flojo. También `docs/interfaces.md` y sus tests.
- **No toques:** `src/oracle/calibra/`, el resto de `src/oracle/calibra.rs`, `src/corre/`, `src/route/`, `src/proveedores/` y `src/config.rs`. En un repo sin esos archivos ni secretos, el veredicto no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La validez automática corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Una consulta de `--help` de gitleaks, `cargo audit` y `cargo clippy` en la imagen está permitida. No corras contenedores de tests.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La revisión de cierre confirmó que un repo evaluado puede apagar sondas, que un build ruidoso pierde su motivo o agota la memoria, que el texto del redactor cae en la acción equivocada, y que la línea de un secreto llega a un proveedor externo.

Leé en el plan, antes de empezar:

- "Verificación de la revisión de cierre (Opus, sobre laptop/tarea-69)".
- "Desempates técnicos 41 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **el repo evaluado no decide qué mide Root, y un secreto no sale de la máquina.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo donde se indica. Las pruebas con herramienta real corren solo con podman: las corre el orquestador, y el informe lo dice.

1. **Salida del build.** `correr_comando` lee con cola de 1 MiB, no con `read_to_end`. La clasificación de un build usa esa cola, y `recortar_salida` corre recién al guardar. Rojo:
   - un `RUN` que falla con 8 KB de salida después de su `STEP` hoy da `Infra`, y debe dar "la imagen no construye";
   - una salida de más de 1 MiB no hace crecer el búfer.
2. **Gitleaks y audit.** Las dos corren sobre una copia del repo sin `.gitleaks.toml`, `.gitleaksignore` ni `.cargo/audit.toml`. Gitleaks ignora `gitleaks:allow`. Leé `--help` de cada una en la imagen antes de elegir flags, y escribí en el informe cuáles usaste. Rojo: el comando de hoy corre sobre `/repo` y no quita nada. Suma una guarda con la herramienta real: un secreto con esos archivos presentes da hallazgo.
3. **Clippy.** Reemplazá `-W` por `--force-warn` para `clippy::correctness`, `clippy::suspicious` y `clippy::perf`. Rojo: el comando de hoy no lo lleva. Suma una guarda con la herramienta real: `#![allow(clippy::all)]` no apaga el hallazgo. Si `--force-warn` no existe en la imagen, detenete y escribilo en el informe.
4. **`/salida`.** Un solo lector de junit y de la lista de tests: usa `symlink_metadata`, rechaza lo que no es archivo normal y tiene tope de tamaño. Rojo: un junit que es un enlace a otro archivo hoy se lee, y debe dar el mismo resultado que un junit ausente.
5. **Redactor.** El texto del redactor cae en la acción que el redactor vio. Guardá los índices que `ocultar_tests` conserva, o aplicá con otro método que mantenga el orden. Rojo: con el primer riesgo oculto, el primer texto hoy cae en el riesgo oculto.
6. **Juez sin secretos.** En `extractos`, la línea de un hallazgo de `gitleaks` se reemplaza por `<n>: [línea con un secreto omitida]`, en todo archivo. Rojo: un archivo citado por gitleaks hoy manda su línea al prompt. Guarda: sin hallazgo de gitleaks, el prompt es idéntico al de hoy, byte por byte.
7. **Test flojo.** En `calibra_cuatro_hilos_los_casos_con_reserva_no_suman_dos_veces`, `gasto` y `con_reserva` leen la misma foto de reservas. Quitá el `sleep` de 50 ms entre ellas. El código de producción no cambia. Es una guarda: corré el test 50 veces con carga, y escribí el resultado en el informe.

No corras la calibración ni la validez. Las corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-71.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
