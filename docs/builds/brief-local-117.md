# Brief de la Tarea 117 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `claude/root-e7813e` en 64361fa. Ahí ya están mezcladas las Tareas 84 a 109.
- La 113 se construye a la vez, en `laptop/tarea-113`. Toca `src/oracle/informe.rs` y `src/oracle/calibra.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-117`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, `cp -a` en las cuatro copias | `src/oracle/calibra/target_podman.rs`, `src/oracle/calibra/defectos.rs`, `src/oracle/descarga.rs` |
| 2, el perfil sin debuginfo | `src/oracle/contenedor.rs` |
| 3, la medida de la ganancia | Un test nuevo en `src/oracle/calibra/target_podman.rs`, y el informe |

- Los puntos 1 y 2 no comparten archivos. Corren en paralelo. El punto 3 va después de los dos.
- **No toques:** `src/oracle/informe.rs`, `src/oracle/calibra.rs`, `src/oracle/mod.rs`, `src/tipos.rs`, `src/lib.rs`, `src/limpieza.rs`, `src/corre/`, el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, P106, literal:** "b Y c". La parte c pide que Root mismo sea más eficiente.

**`cp -r` pone fechas nuevas.** cargo compara la fecha de cada fuente con su huella. Con fechas nuevas, recompila el crate aunque el `target` venga sembrado. `cp -a` conserva las fechas. Root ya usa `cp -a` en `copiar_target` (`src/oracle/mod.rs:2094`) y en la mutación (`src/oracle/mutacion.rs:643` y `:694`).

**Los cuatro `cp -r` que quedan:**
- `src/oracle/calibra/target_podman.rs:17`, `COMANDO_OCULTO_CON_TARGET`, de un test con podman.
- `src/oracle/calibra/defectos.rs:327`, `COMANDO_EFICIENCIA`, de un test con podman.
- `src/oracle/calibra/defectos.rs:429`, `COMANDO_OCULTO`, de un test con podman.
- `src/oracle/descarga.rs:141`, `COMANDO_LOCK`, que genera el `Cargo.lock`.

**Este punto no gana tiempo en producción.** Las suites de producción corren sobre `/repo` sin copiarlo. El punto deja una sola regla de copia en todo Root. No cambies `PRELUDIO_PYTHON` (`descarga.rs:75`): copia fuentes de Python y no compila.

**La información de depuración cuesta compilar y enlazar.** La documentación de cargo-mutants sugiere apagarla en las corridas de tests. Las variables `CARGO_PROFILE_DEV_DEBUG=0` y `CARGO_PROFILE_TEST_DEBUG=0` la apagan sin tocar el `Cargo.toml` del producto. El perfil de release no cambia.

**El riesgo es recompilar todo.** El `target` sembrado guarda la huella del perfil. Si un comando lleva las variables y otro no, cargo recompila todo en cada cambio de comando. Por eso las variables van en un solo lugar: `opciones` (`src/oracle/contenedor.rs:141`), que arma los `-e` de todo contenedor.

## Tarea

La regla que manda: **Root compila menos, y ningún resultado cambia.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. `cp -a` en las cuatro copias

1. Cambiá `cp -r` por `cp -a` en las cuatro constantes. Mantené el resto del comando igual.
2. En `COMANDO_LOCK`, la forma es `cp -a /repo/. /tmp/w/`.
3. Actualizá el test de `descarga.rs` que lee el literal `cp -r /repo/. /tmp/w/`, cerca de la línea 2000.

**Tests en rojo primero:**
- Unitario: ninguna de las cuatro constantes contiene `cp -r`.
- Guarda: `COMANDO_LOCK` sigue copiando `/repo/.`, con `.cargo/` incluido, a `/tmp/w/`.
- Guarda con podman, para el orquestador: `target_podman_reusado_da_la_misma_completitud_que_en_frio` sigue verde. Anotá su nombre en el informe.

### 2. El perfil sin debuginfo

1. **Una constante** lista los dos pares: `CARGO_PROFILE_DEV_DEBUG=0` y `CARGO_PROFILE_TEST_DEBUG=0`. Su comentario da el motivo: P106 c, y la misma huella para todo comando sobre el `target` sembrado.
2. **`opciones`** suma esos dos `-e` en todo contenedor, antes de los de `e.env`.
3. **Un valor de `e.env` manda.** Si una ejecución pone su propio valor para una de esas claves, ese par queda después, y podman usa el último.
4. Los contenedores de Python también reciben las variables. Ahí no cambian nada.

**Tests en rojo primero:**
- Unitario: `argumentos` y `argumentos_separado` llevan `-e CARGO_PROFILE_DEV_DEBUG=0` y `-e CARGO_PROFILE_TEST_DEBUG=0`.
- Unitario: con un `e.env` que pone `CARGO_PROFILE_DEV_DEBUG=2`, ese par queda después del par de la constante.
- Guarda: los tests de endurecimiento y montajes de `contenedor.rs` siguen verdes.

### 3. La medida de la ganancia, y la guarda de resultado

Un test nuevo en `target_podman.rs`, marcado `#[ignore]`, compila en el host el fixture de calibración. Usa `copia_fixture`, que ese archivo ya importa. No necesita podman. Corrélo a mano una vez, con `--ignored`.

1. **Lado A:** `cargo test --offline` en una copia del fixture, sin las variables, con un `target` vacío.
2. **Lado B:** lo mismo en otra copia, con las dos variables, y con otro `target` vacío.
3. **El resultado no cambia:** los dos lados dan los mismos tests, con el mismo resultado cada uno. El test lo afirma.
4. **La ganancia:** el test imprime el tiempo de compilación de cada lado, y el tamaño de cada `target`. Corré el test 3 veces y anotá el mínimo de cada lado.
5. Las carpetas van en el helper temporal que ya usan los otros tests. Ninguna va a `/tmp` del host.

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

Al terminar, escribí `docs/builds/informe-local-117.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La ganancia medida**: el mínimo de 3 corridas de cada lado, en tiempo de compilación y tamaño del `target`.
5. **Guardas con podman para el orquestador**: el nombre de cada test que debe seguir verde.
6. **Tests que cambiaron de valor**, con su motivo.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
