# Brief de la Tarea 136 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-134`. Trae las Tareas 84 a 134 y la 127b.
- La 135 se construye a la vez. Toca `src/oracle/eficiencia.rs`, `src/oracle/calibra.rs`, `src/tipos.rs` y `docs/interfaces.md`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-136`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el lastre en una función que un test viejo llama | `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra.rs`, `src/oracle/eficiencia.rs`, `src/oracle/mod.rs`, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **El motivo de un lastre sin lugar lo suma la 135,** en `calibra.rs`. Esta tarea solo devuelve `None` sin tocar la copia, como hoy.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La Tarea 134 puso el lastre en una función vieja, y no alcanzó.** Una función vieja existe en `bootstrap/`. Eso no dice que un test la corra.
- **t1:** los lastres cayeron en `src/pattern/composite_pattern.rs:19` y `:170`. La memoria pico no cambió: 42 MB, razones de 0,991 y 0,992.
- **t3:** cayeron en `src/content_search/content_match.rs:17` y `src/tree_build/bline.rs:42`. La memoria no cambió: razones de 1,000 y 0,998.
- **t2:** cayeron en `src/content_search/mod.rs:32` y `:44`, funciones que los tests viejos de ese módulo llaman. La memoria subió 2,35 y 2,40 veces.

**Por qué importa quién llama.** La medición automática de Eficiencia solo corre los tests que pasan en `bootstrap/` y en el repo. Un test que ya existía en `bootstrap/` y pasaba ahí sigue pasando en el repo, salvo que el cambio lo rompa. Esos tests viejos son, casi todos, los tests comunes. Una función que un test viejo llama es una función que la medición corre.

**Todo sale del texto, sin correr nada.** Los tests de Rust son funciones con `#[test]`. Sus cuerpos llaman a funciones por su nombre.

## Tarea

La regla que manda: **el lastre va a una función vieja de producción que un test viejo llama por su nombre.** Hacé un commit. Empieza con su test en rojo.

### 1. El lastre en una función que un test viejo llama

1. **Un test viejo** es una función `#[test]` que existe en `bootstrap/`, en el mismo archivo y con el mismo nombre que en el repo. Vale en `src/` (módulos de tests) y en `tests/`.
2. **Los nombres llamados por un test viejo** son los identificadores seguidos de `(` en su cuerpo, en `bootstrap/`. Por ejemplo, `foo(` y `.foo(`. Sacá las palabras clave y las macros, como `assert!` y `vec!`.
3. **Una función candidata** es una función vieja de producción (la regla de la 134) cuyo nombre está en esos nombres llamados.
4. **El lugar del lastre,** en orden:
   - una función candidata de un archivo del cambio;
   - si no hay, una función candidata de cualquier archivo de producción;
   - si no hay, ninguna. El inyector devuelve `None` sin tocar la copia.
5. **Las demás reglas del lugar no cambian.** No vale en una función `const`, en un bloque `unsafe`, en código de tests ni en un `main` de ejemplo.
6. **Una función pura** recibe los textos de `bootstrap/` y da los nombres llamados por los tests viejos. Otra dice si una función es candidata.
7. **Sin `bootstrap/`,** el lugar es el de antes de la 134.

**Tests en rojo primero:**
- Unitario: en un repo de prueba con dos funciones viejas en un archivo del cambio, y un test viejo que llama solo a una, el lastre va a la llamada. Hoy puede ir a la otra.
- Unitario: un test nuevo, que no está en `bootstrap/`, no cuenta como test viejo.
- Unitario: `assert_eq!(` y `vec!(` no cuentan como nombres llamados.
- Unitario: sin funciones candidatas, el inyector devuelve `None` sin tocar la copia.
- Unitario: el repo inyectado compila con `cargo check --offline`.
- Guarda: los tests de `LastreDeMemoria` de la 131 y la 134 siguen verdes, salvo los que fijaban el lugar viejo. Anotá esos en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 135 y la 136. En t1 y t3, los defectos deben subir la memoria, como en t2. Si un caso no la sube, sus medidas lo muestran, y el informe del orquestador anota el lugar elegido.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-136.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El lugar del lastre en el repo de prueba**: el archivo, la línea, la función, y el test viejo que la llama.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
