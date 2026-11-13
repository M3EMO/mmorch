# Brief de la Tarea 108 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-107` (d7dfcb0). Trae la 107 y la 107b. Las Tareas 84 a 107 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-108`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el plazo vencido como variante | `src/oracle/mod.rs`, `src/oracle/contenedor.rs` |
| 2, ningún plazo aborta la calibración | `src/oracle/calibra.rs` |
| 3, `corre` usa `limpieza.rs` | `src/corre/barrido.rs`, `src/limpieza.rs` |

- El punto 2 usa la variante del punto 1. Van en serie.
- El punto 3 corre en paralelo.
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

**Un plazo de Root vencido todavía aborta la calibración.** Preguntas 6 y 7 del informe de la 107 (`/home/mpera/.cache/root-wt/local107/docs/builds/informe-local-107.md`):
- Un caso con defecto cuyo plazo de Root vence, con junit presente, aborta toda la calibración.
- Un limpio cuyo plazo vence aborta toda la calibración, porque no tiene inyección que reemplazar.

**El plazo se reconoce por texto.** Pregunta 8: `contenedor.rs:217` escribe "timeout tras {} s; contenedor {nombre} eliminado", y `mod.rs:2215` lo reconoce con `PREFIJO_VENCIMIENTO`. Un cambio de texto rompería la regla en silencio.

**La regla de pid vivo está duplicada, y una copia tiene el defecto de la 107b.** `src/limpieza.rs` tiene `r00t_vivo` con las tres reglas de la 107b: el pid propio, el mismo ejecutable por `/proc/<pid>/exe`, o un nombre `r00t` o `r00t-*`. `src/corre/barrido.rs:199` tiene su propio `es_r00t`, con la regla vieja. Con esa regla, un `r00t-<sha>` vivo cuenta como muerto. Ese barrido borra credenciales, así que puede dejar sin credencial a una corrida viva.

## Tarea

La regla que manda: **un plazo vencido le dice algo al caso, no a toda la calibración; y la regla de pid vivo existe una sola vez.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El plazo vencido como variante

1. `ErrorOraculo` suma una variante para el plazo vencido de un contenedor. Lleva el plazo y el nombre del contenedor.
2. `contenedor.rs` devuelve esa variante donde hoy arma el texto "timeout tras".
3. `mod.rs` deja de comparar con `PREFIJO_VENCIMIENTO`, y mira la variante. Borrá la constante si queda sin uso.
4. El mensaje que ve el usuario no cambia.

**Tests en rojo primero:**
- Unitario: un plazo vencido de `ejecutar` da la variante nueva, con el plazo y el nombre.
- Unitario: `mod.rs` reconoce la variante, aunque su texto cambie.
- Guarda: el texto que se muestra es el de hoy.

### 2. Ningún plazo de Root aborta la calibración

1. **Caso con defecto.** Si el plazo de Root vence, con o sin junit, el caso es una inyección inválida. Va a reemplazo, con el motivo "la suite pasó su plazo".
2. **Limpio.** Si el plazo de Root vence, el limpio queda sin veredicto, con el motivo "la suite pasó su plazo". Cuenta en `limpios_sin_veredicto`. No aborta.
3. **La regla de hoy sigue:** con más de 20 % de limpios sin veredicto, la dimensión no calibra (`MAX_LIMPIOS_SIN_VEREDICTO_PCT`).
4. **Base.** Si el plazo de la base vence, la calibración sigue abortando, como hoy.

**Tests en rojo primero:**
- Unitario: un caso con defecto cuyo plazo vence con junit presente va a reemplazo, y la calibración sigue.
- Unitario: un limpio cuyo plazo vence queda sin veredicto, con su motivo, y la calibración sigue.
- Unitario: con 2 de 5 limpios vencidos, la dimensión no calibra.
- Guarda: una base cuyo plazo vence sigue abortando.

### 3. `corre` usa `limpieza.rs`

1. `corre::barrido` usa `r00t_vivo` de `src/limpieza.rs` para saber si un pid es de un r00t vivo. Hacelo `pub(crate)` si hace falta.
2. `corre::barrido` conserva su chequeo de hora de inicio contra los segundos del id de la corrida, con su margen de 300 s.
3. Borrá `es_r00t` de `src/corre/barrido.rs`, después de unir sus llamadores.

**Tests en rojo primero:**
- Unitario: una corrida de un `r00t-<sha>` vivo no se barre. Hoy se barre.
- Guarda: los tests de barrido de `corre` de las Tareas 84, 85 y 87 siguen verdes, incluido el margen de 300 s.

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

Al terminar, escribí `docs/builds/informe-local-108.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
