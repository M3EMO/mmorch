# Brief de la Tarea 118 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-117`, en e19886e. Trae las Tareas 84 a 114 y la 117.
- La 116 y la 119 pueden construirse a la vez. La 116 toca `src/corre/` y `src/tipos.rs`. La 119 toca `src/oracle/eficiencia.rs`, `src/oracle/perfiles/herramientas.rs`, `src/oracle/mod.rs` y `src/oracle/informe.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-118`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el barrido no corre sin podman | `src/limpieza.rs` |
| 2, la auditoría | Solo el informe. Sin cambios de código |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/corre/`, `src/tipos.rs`, `src/entorno.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Un test sin podman llama al podman real.** `contenedor::ejecutar` (`src/oracle/contenedor.rs:225`) llama a `crate::limpieza::barrer_al_arrancar("podman")`. `barrer_antes_de_arrancar` (`src/oracle/entrega.rs:1613`) hace lo mismo. `barrer_al_arrancar` (`src/limpieza.rs:37`) lista y quita contenedores con el podman real.

**El daño medido.** La suite con podman de la Tarea 110 dio 137 dos veces mientras un builder corría su suite sin podman en otro worktree. Ese builder salía de una base sin la Tarea 112. Su barrido usaba la regla vieja del nombre, y mató contenedores vivos de la otra suite.

**La 112 no alcanza.** Desde la 112, el barrido respeta a un dueño vivo. Pero con `ROOT_SIN_PODMAN=1` un test promete no usar podman. Cada barrido igual corre `podman ps`, y eso pesa en los tests de plazo bajo carga.

**La regla ya existe.** `entorno::debe_saltear` (`src/entorno.rs:9`) da verdadero solo con `Some("1")`. La variable es `entorno::VARIABLE_SIN_PODMAN`.

## Tarea

La regla que manda: **con `ROOT_SIN_PODMAN=1`, el barrido no llama a podman.** Hacé un commit por punto. El punto 1 empieza con su test en rojo.

### 1. El barrido no corre sin podman

1. `barrer_al_arrancar` vuelve enseguida si `entorno::debe_saltear` da verdadero con el valor de `VARIABLE_SIN_PODMAN`.
2. Es un solo lugar. No cambies a `contenedor::ejecutar` ni a `entrega.rs`.
3. Vale también en el binario. Los tests de `tests/` corren `r00t` como proceso aparte, y ese proceso lee la misma variable.
4. **La lectura de la variable es inyectable** para los tests. Un test no cambia el entorno del proceso, porque los tests corren en paralelo.
5. `barrer`, con su programa y su raíz de `/proc`, no cambia. Los tests de la 107, la 107b y la 112 lo llaman directo.

**Tests en rojo primero:**
- Unitario: con la variable en `"1"`, el barrido no llama a su programa. Usá un programa falso que deja una marca en una carpeta temporal. Hoy la marca aparece.
- Unitario: con la variable ausente, el barrido llama a su programa.
- Unitario: con la variable en `"0"` o en `"true"`, el barrido llama a su programa. Es la misma regla de `debe_saltear`.
- Guarda: los tests de barrido de la 107, la 107b y la 112 siguen verdes.

### 2. La auditoría

Sin cambios de código. El informe lista cada test sin `saltar_sin_podman` que llega a un podman real.

1. **Los lugares con el literal `"podman"`,** en e19886e:
   - `src/oracle/contenedor.rs:282`, `:362`, `:444` y `:697`.
   - `src/oracle/descarga.rs:1305`.
   - `src/oracle/entrega.rs:208`.
   - `src/oracle/calibra/target.rs:52`.
   - `src/corre/ejecutor.rs:242`.
   - `src/corre/corrida.rs:796`.
2. **Por cada lugar,** uní los llamadores de codegraph y de `grep -w`. Seguí la cadena hasta los tests.
3. **Por cada test** que llega ahí sin `saltar_sin_podman`, anotá el nombre, el archivo y el camino de llamadas.
4. **Una forma de comprobarlo:** corré la suite sin podman con un `podman` falso al frente del `PATH`. El falso anota cada llamada en un archivo dentro de tu worktree. Anotá en el informe qué tests lo llamaron.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- `listar_lee_la_hora_de_creacion_de_podman_real` (`src/limpieza.rs:705`), y cada test de barrido con `saltar_sin_podman`.
- **La prueba de convivencia.** El orquestador corre la suite con podman de la rama mientras un builder corre la suite sin podman en otro worktree. Ningún test con podman debe dar 137.

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

Al terminar, escribí `docs/builds/informe-local-118.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La auditoría**: cada test que llega al podman real sin `saltar_sin_podman`, con su archivo y su camino de llamadas.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
