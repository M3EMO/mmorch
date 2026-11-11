# Brief de la Tarea 97 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Base: punta de laptop/tarea-96. El commit es `7b65814`.
- Las Tareas 84 a 96 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-97`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, estado por mutante | `src/oracle/mutacion.rs`, `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 2, plazo fijo de cargo-mutants | `src/oracle/mutacion.rs`, `src/oracle/calibra.rs` |
| 3, muestra dentro del cambio | `src/oracle/mutacion.rs` |

- Los tres puntos tocan `mutacion.rs`. Van en serie: 1, 2 y 3.
- `src/oracle/calibra.rs` también lo tocó la Tarea 96. Esta tarea arranca sobre su punta, así que no hay choque.
- Si el punto 3 necesita los rangos desde `src/oracle/mod.rs`, tocalo lo mínimo y explicalo en el informe.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Pruebas en t2: el puntaje subió al apagar tests.** El orquestador calibró sin API en t2 de S2. `assert_borrado` apagó tres tests de `fuzzy_pattern_tests`, los que mataban al mutante `src/pattern/fuzzy_pattern.rs:126:68: replace - with + in FuzzyPattern::tight_match_from_index`. El puntaje de mutación pasó de 1 de 15 a 2 de 15. El limpio dio 1 de 15, igual que la base.

**Cómo mide hoy la sonda** (`src/oracle/mutacion.rs`):
- Lista los mutantes de la copia (`comando_de_listado`), y toma 15 repartidos en toda la lista (`muestra_repartida`, línea 366).
- Corre solo esos, con `-F` (`comando_de_corrida`, línea 384), con el plazo automático de cargo-mutants.
- `leer_viables` cuenta `CaughtMutant` y `Timeout` como muertos.
- Los casos de calibración no guardan el estado de cada mutante. Por eso no se puede saber cuál cambió.

**Causa más probable.** cargo-mutants fija el plazo de cada mutante según cuánto tarda la corrida de base de esa misma ejecución. Con tres tests apagados, la base tarda menos y el plazo baja. Un mutante lento que antes sobrevivía ahora vence el plazo, y Root lo cuenta como muerto. El limpio no apaga tests, así que no lo ve. Falta el dato para confirmarlo.

**La muestra cae fuera del cambio.** El puntaje base de t2 es 1 de 15, y el de t3 es 1 de 12. La muestra repartida cae casi toda en código que los tests ocultos no cubren. Root evalúa el cambio de un builder. El puntaje de Pruebas debe hablar de ese cambio.

## Tarea

La regla que manda: **el puntaje de Pruebas mide los tests sobre el cambio, y una herramienta ruidosa se ve, no se esconde.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Estado por mutante

1. `ResultadoMutacion` suma la lista de la muestra con el estado de cada mutante: muerto, vivo o vencido. `Timeout` queda como vencido. El campo lleva `#[serde(default)]`.
2. El puntaje no cambia: vencido sigue contando como muerto.
3. Cada caso de calibración de Pruebas guarda esa lista.
4. Cada caso guarda también `mutantes_cambiados`: los mutantes de la muestra que cambiaron de estado frente a la base, sin contar el mutante elegido por `assert_borrado`.
5. Un caso con `mutantes_cambiados` no vacío queda marcado como ruidoso. Sigue contando para las métricas.
6. La fila suma la cantidad de casos ruidosos.
7. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un `outcomes.json` con un mutante muerto, uno vivo y uno vencido da los tres estados en la lista.
- Unitario: un caso cuya lista difiere de la base en un mutante que no es el elegido lo lista en `mutantes_cambiados`, y queda ruidoso.
- Unitario: la fila cuenta los casos ruidosos.
- Guarda: el puntaje de un `outcomes.json` de hoy no cambia.
- Guarda: un caso viejo sin los campos se lee con listas vacías.

### 2. Plazo fijo de cargo-mutants

1. La corrida de base de la calibración guarda cuánto tardó su escenario `Baseline` de cargo-mutants. Sale del `outcomes.json`. Si el reporte no lo trae, explicá de dónde lo sacaste.
2. Cada caso de esa calibración corre con `--timeout <segundos>` fijo, calculado de esa duración con el mismo multiplicador que usa cargo-mutants por defecto. Anotá en el informe el multiplicador y su fuente.
3. Así, apagar tests ya no cambia el plazo de cada mutante.
4. Fuera de la calibración, la sonda sigue con el plazo automático.

**Tests en rojo primero:**
- Unitario: el comando de un caso trae `--timeout` con el valor calculado desde la base.
- Unitario: el comando de una evaluación sin calibración no trae `--timeout`.

### 3. Muestra dentro del cambio

1. Si la tarea trae `bootstrap/`, la muestra sale de los mutantes cuya línea cae dentro de los rangos cambiados, calculados con `cambios::rangos_cambiados` entre el estado inicial y el repo.
2. Dentro del cambio, la muestra sigue repartida, con el mismo tope.
3. Sin mutantes dentro del cambio, Pruebas no tiene veredicto, con el motivo "ningún mutante cae dentro del cambio".
4. Sin `bootstrap/`, la muestra repartida de hoy no cambia.
5. Esto vale para la evaluación y para la calibración.
6. `assert_borrado` elige su mutante de los muertos de la base. Con esta regla, ese mutante ya cae dentro del cambio.

**Tests en rojo primero:**
- Unitario: con rangos que cubren solo un archivo, la muestra trae solo mutantes de ese archivo y de esas líneas.
- Unitario: con rangos sin mutantes, Pruebas queda sin veredicto, con su motivo.
- Unitario: sin rangos, la muestra es la de hoy.

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

Al terminar, escribí `docs/builds/informe-local-97.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Plazo de cargo-mutants**: de dónde sale la duración de la base, el multiplicador y su fuente.
3. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
