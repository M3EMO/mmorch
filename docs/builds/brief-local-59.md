# Brief de la Tarea 59 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (367b0b4 o posterior). Otros builders trabajan en paralelo: la 55 en `src/corre/` y en `comando_git` y `commit_de` de `src/oracle/mod.rs`, y la 57 en la descarga.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-59`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, y en `src/oracle/mod.rs` solo lo necesario para que la evaluación de un caso reciba un `target` inicial. También sus tests y `docs/interfaces.md`.
- **No toques:** `comando_git` y `commit_de` de `src/oracle/mod.rs`, `src/corre/`, `src/oracle/descarga*`, `src/oracle/contenedor.rs`, `src/oracle/entrega.rs`, `src/oracle/memo.rs`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Cada caso de calibración compila memora o brasa desde cero en su propio `/salida/target`. El orquestador midió memora-ecc: `cargo test --no-run` tarda 80 s en frío y 4,7 s con el `target` de la base. Son unas 10 horas de CPU en toda la calibración. Además, los tests inestables de la base pueden dar falsos positivos en los casos limpios.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04 (madrugada): Tarea 54 mezclada, humo y compilación".
- "Desempates técnicos 30 (Opus, 2026-11-04): reuso del `target` y reparto de la calibración".
- "Tarea 59: Reuso del `target` de la base y detección sin inestables".

## Tarea

La regla que manda: **un caso de calibración mide lo mismo que una evaluación real.** Solo cambia el tiempo. Hacé un commit por punto.

1. **`target` de la base.** Después de evaluar la base, la calibración guarda su `/salida/target` en la carpeta temporal de la calibración.
2. **Copia por caso.** Cada caso arranca con una copia de ese `target`, hecha con `cp --reflink=auto`, en el mismo `/salida/target` del contenedor. Las rutas que cargo guarda no cambian.
3. **Cuándo compila en frío.** Si el caso cambia `Cargo.toml` o `Cargo.lock`, si cambia el digest de la imagen, o si la base no dejó `target`, el caso arranca con un `target` vacío. Su renglón de `.casos.jsonl` suma `target_reusado: true` o `false`.
4. **Solo en la calibración.** La evaluación real no recibe un `target` de otra evaluación. No cambies ese camino.
5. **Inestables fuera de la detección.** En Completitud y Funciona, un caso detecta un defecto solo por tests que la base clasificó como estables. Los inestables de la base no cuentan para detectar, ni en un caso con defecto ni en uno limpio.
6. **Tests.**
   - Sobre un fixture Rust, un caso con el `target` reusado y el mismo caso en frío dan el mismo veredicto. Usá podman.
   - Un caso que cambia `Cargo.lock` compila en frío, y su renglón dice `target_reusado: false`.
   - Un test inestable de la base que cambia de clase en un caso limpio no cuenta como detección.
   - Anotá en el informe cuánto tarda un caso de Completitud del fixture, en frío y con el `target` reusado.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-59.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
