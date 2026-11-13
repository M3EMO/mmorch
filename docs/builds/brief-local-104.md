# Brief de la Tarea 104 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-103` (22d0732). Trae la 103 y la 103b, sobre la 102. Las Tareas 84 a 103 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-104`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, más fuentes en la línea base | `src/oracle/previos.rs`; `src/oracle/mod.rs` solo si la marca de previo de esas fuentes se aplica ahí |
| 2, comentario de `EXCLUIR_DELETE_FIELD` | `src/oracle/mutacion.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P96, literal:** "a". Es la opción "Bloquea solo lo que el cambio introduce. Lo previo se informa, y no bloquea. Un secreto bloquea siempre."

**Fuentes que la Tarea 103 dejó afuera.** La línea base (`src/oracle/previos.rs`, `linea_base` en la línea 40) corre cargo audit, gitleaks y las sondas nativas. El informe de la 103 dice que las sondas de configs, hadolint y la descarga nunca son previo. Esas sondas también dan hallazgos de Seguridad, por ejemplo en `src/oracle/configs.rs:278` y `:461`. Un hallazgo de severidad 3 que ya estaba en el estado inicial bloquearía igual. Eso contradice la respuesta del usuario.

**El comentario de `EXCLUIR_DELETE_FIELD`** (`src/oracle/mutacion.rs:29`) dice que esos mutantes se excluyen con `--exclude-re`. El orquestador midió que cargo-mutants 27.1.0 también ignora `--exclude-re` para ese tipo. `fuera_de_la_muestra` los saca del puntaje, pero corren igual.

## Tarea

La regla que manda: **toda fuente de Seguridad sigue la misma regla de lo previo, y un comentario dice lo que pasa de verdad.** Hacé un commit por punto. El punto 1 empieza con su test en rojo.

### 1. Más fuentes en la línea base

1. `linea_base` corre también, sobre `oraculos/<tarea>/bootstrap/`, las sondas de Seguridad de configs, hadolint y la descarga. Usa los mismos comandos que la evaluación.
2. La marca de previo aplica a esos hallazgos con la regla de id de la 103b: id, severidad y, si hay, el id del aviso.
3. **Los secretos bloquean siempre.** Un secreto que esas sondas encuentran, por ejemplo en un YAML, queda marcado como previo si estaba, pero bloquea igual.
4. La clave de la memoria de la línea base cambia, porque la línea base tiene más fuentes. Una línea base vieja no se reusa.
5. Una sonda nueva que da sin veredicto sobre `bootstrap/` deja la línea base sin guardar, como hoy con las otras fuentes.

**Tests en rojo primero:**
- Unitario: un hallazgo de Seguridad de severidad 3 de la sonda de configs, igual en `bootstrap/` y en el repo, queda previo y no bloquea.
- Unitario: el mismo hallazgo, solo en el repo, queda nuevo y bloquea.
- Unitario: un secreto de configs presente en `bootstrap/` queda previo y bloquea igual.
- Unitario: una línea base guardada con la clave vieja no se reusa.
- Guarda: los tests de la 103 y la 103b siguen verdes.

### 2. El comentario de `EXCLUIR_DELETE_FIELD`

1. El comentario dice que cargo-mutants 27.1.0 ignora `-F` y `--exclude-re` para los mutantes "delete field ... from struct ... expression", que esos mutantes corren igual, y que `fuera_de_la_muestra` los saca del puntaje.
2. `--exclude-re` se queda, para una versión de cargo-mutants que lo respete.

Este punto no cambia comportamiento. La guarda es la suite verde.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-104.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Fuentes de la línea base**: cuáles corre ahora y con qué clave se guarda.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
