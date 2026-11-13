# Brief de la Tarea 114 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-111`, en 04cf8e7. Trae las Tareas 84 a 111.
- La 113 está en `laptop/tarea-113`, y no está en tu base. La 117 se construye en `laptop/tarea-117`. No toques `src/oracle/informe.rs`, `src/oracle/contenedor.rs` ni `src/oracle/descarga.rs`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-114`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el `home` de la evaluación | `src/oracle/mod.rs` (solo `evaluar_con`), `src/oracle/calibra.rs` |
| 2, la vigencia de `inestables_de` | `src/oracle/calibra.rs` |
| 3, los lectores de "¿tiene valor?" | `src/oracle/calibra.rs`, `src/oracle/calibra/*.rs`, `src/oracle/validez.rs` |

- Los tres puntos tocan `calibra.rs`. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/tipos.rs`, `src/oracle/informe.rs`, `src/lib.rs`, el resto de `src/oracle/mod.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La evaluación lee la calibración desde el `ROOT_HOME` global, no desde el suyo.** `evaluar_con` (`src/oracle/mod.rs:581`) arma su `home` en la línea 595, desde `o.home` o `root_home()`. Pero llama a `deps.calibracion.aplicar` (línea 909) y a `motivo_sin_calibrar` (línea 921). Las dos funciones leen `crate::config::root_home()` (`src/oracle/calibra.rs:230` y `:310`). Una evaluación con otro `home` lee los `tarea.toml` de otras tareas desde el `ROOT_HOME` global.

**Varios tests leen el `ROOT_HOME` real.** Los tests de `calibra.rs` que llaman a `aplicar` y a `motivo_sin_calibrar` leen `root_home()`. Su resultado depende de los datos del usuario. El orquestador está por recalibrar ese `ROOT_HOME`, y esos tests pueden cambiar de valor sin un cambio de código.

**`inestables_de` acepta filas viejas.** `inestables_de` (`src/oracle/calibra.rs:294`) filtra con `huella_vigente` y `tarea_vigente`.
- `huella_vigente` (línea 577) acepta una fila sin huella en una dimensión sin juez.
- `tarea_vigente` (línea 542) acepta una fila sin `tarea_hash`.
- La Tarea 111 decidió que una fila sin huella o sin `tarea_hash` no cuenta para calibrar. Esa fila tampoco debe excluir ítems como inestables.

**Un `no_aplica` se lee como un 0 medido.** La Tarea 110 separó `Puntaje.no_aplica` de `Puntaje.sin_veredicto`. `Puntaje::sin_valor()` (`src/tipos.rs:280`) da verdadero en los dos casos. Varios lectores todavía preguntan solo por `sin_veredicto`. Un puntaje con `no_aplica` y `valor` 0 pasa como medido. Algunos lugares conocidos, en 04cf8e7:
- `global_de`, `src/oracle/calibra.rs:478`.
- `src/oracle/calibra.rs:2736`, `:2754`, `:2787` y `:2841`, sobre el puntaje de un caso.
- `src/oracle/calibra/regla.rs:43`.
- `completitud_de`, `src/oracle/validez.rs:546`.

La lista no es completa. Los lectores de `Caso.sin_veredicto` (líneas 1807, 1812, 1838 y 2871) leen otro tipo. Revisalos igual, y decidí cada uno.

## Tarea

La regla que manda: **la evaluación lee la calibración desde su propio `home`, una fila vieja no excluye ítems, y un `no_aplica` nunca cuenta como medido.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El `home` de la evaluación

1. En `evaluar_con`, la línea 909 llama a `aplicar_en(&home, ...)`, y la línea 921 llama a `motivo_sin_calibrar_en(&home, ...)`.
2. Uní los llamadores de `aplicar` y de `motivo_sin_calibrar`. Si después del paso 1 solo los usan tests, pasá esos tests a la versión con `home`, con una carpeta temporal. Después borrá `aplicar` y `motivo_sin_calibrar`.
3. Si un llamador de producción fuera de `mod.rs` los usa, no los borres. Anotalo en el informe.

**Tests en rojo primero:**
- Unitario: una evaluación con `o.home` en una carpeta temporal lee el `tarea.toml` de otra tarea desde esa carpeta. Hoy lo lee desde `root_home()`.
- Guarda: ningún test de `calibra.rs` llama a una función que lea `root_home()`. Anotá en el informe cómo lo comprobaste.

### 2. La vigencia de `inestables_de`

1. `inestables_de` usa una fila solo si su huella es la actual (`huella_actual`) y su `tarea_hash` es igual al actual.
2. Una fila sin huella o sin `tarea_hash` no sirve. La fila más reciente que sirve da los ítems, como hoy.
3. Borrá `huella_vigente` y `tarea_vigente` si quedan sin uso, después de unir sus llamadores.

**Tests en rojo primero:**
- Unitario: una fila sin `tarea_hash` no da inestables. Hoy los da.
- Unitario: una fila sin huella, en una dimensión sin juez, no da inestables. Hoy los da.
- Guarda: los tests `inestables_de_*` de la 83 y la 111 siguen verdes, salvo los que fijaban la aceptación de filas viejas. Anotá esos en el informe.

### 3. Los lectores de "¿tiene valor?"

1. **Buscá cada lector.** Uní codegraph y `grep -w sin_veredicto` en `src/oracle/calibra.rs`, `src/oracle/calibra/` y `src/oracle/validez.rs`.
2. **En cada lector de un `Puntaje`** que pregunta si la dimensión tiene valor, usá `p.sin_valor()` o `!p.sin_valor()`.
3. **Un lector que necesita el motivo** de `sin_veredicto` sigue leyendo `sin_veredicto`.
4. **Los lectores de `Caso.sin_veredicto`** leen otro tipo. Decidí cada uno.
5. El informe lista cada lugar y su decisión.

**Tests en rojo primero:**
- Unitario: un caso cuya dimensión tiene `no_aplica` y `valor` 0 no se detecta como defecto.
- Unitario: `global_de` no cuenta un puntaje con `no_aplica`, aunque traiga `calibrada` en verdadero.
- Unitario: `completitud_de` en `validez.rs` da `None` para una Completitud con `no_aplica`.
- Un test por cada otro lugar que cambie de valor.

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

Al terminar, escribí `docs/builds/informe-local-114.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lectores revisados**: cada lector de `sin_veredicto` y su decisión.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
