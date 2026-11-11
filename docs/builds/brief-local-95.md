# Brief de la Tarea 95 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-94` (03308b9). Las Tareas 84 a 94 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-95`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, inyección dentro del cambio | `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 2, suite que no compila | `src/oracle/mod.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
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

El orquestador calibró sin API en t1 de S2, con el binario de la Tarea 94.

**Funciona, 0 de 1.** El defecto `comportamiento_roto` cayó en `src/verb/verb_store.rs:667`. Ni los 4 tests ocultos ni los 43 del contrato pasan por esa línea. El valor quedó en 1,0, igual que la base. `comportamiento` (`src/oracle/calibra/inyectar.rs:848`) elige una línea con `==` en cualquier archivo de `src/`. En memora y brasa la suite cubre casi todo el repo, así que eso funcionaba. En una tarea minada, los tests ocultos cubren el cambio del commit. Una línea al azar de broot casi nunca cae ahí. Ese caso mide la cobertura de la tarea, no a Root.

**Completitud, sin veredicto.** El defecto `modulo_borrado` borró `src/keys.rs`. El build se rompió, nextest salió con 101, y el junit no trajo fallos. `juzgar_junit` (`src/oracle/mod.rs:2341`) dio "sin veredicto": "el código del comando (101) no coincide con las fallas del junit". Un repo que no compila debería dar rojo.

**Los códigos de nextest.** nextest documenta un código por clase de fallo. El 100 es tests que fallaron, y el 101 es un build que falló. Verificá esos valores en la documentación de la versión de nextest de la imagen `:3`, y anotá la fuente en el informe.

## Tarea

La regla que manda: **una calibración mide a Root, no la cobertura de la tarea; y un repo que no compila es rojo, nunca verde.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Inyección dentro del cambio

1. **Rangos.** Si la tarea trae `bootstrap/`, `calibra.rs` calcula los rangos cambiados entre el estado inicial y el repo bueno con `cambios::rangos_cambiados` (Tarea 81). Los pasa a `inyectar_con` en `Contexto`.
2. **`comportamiento`** elige una línea con `==` de producción dentro de esos rangos.
3. **`modulo_borrado`** elige un módulo cuyo archivo tiene una línea dentro de esos rangos.
4. **Sin candidatos dentro del cambio,** el defecto no aplica en esa tarea, con el motivo "ninguna línea del cambio admite este defecto". No cae a una línea al azar.
5. **Sin `bootstrap/`,** los dos defectos eligen como hoy.
6. **Marca.** Cada caso con defecto guarda `en_cambio: bool`, con `#[serde(default)]`. Vale `true` si la línea elegida cae dentro del cambio.
7. Los demás defectos no cambian. `AssertBorrado` sigue usando `Contexto.muertos`.
8. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: con rangos que cubren solo las líneas 10 a 20 de un archivo, `comportamiento` elige una línea entre 10 y 20.
- Unitario: con rangos que no traen ninguna línea con `==`, `comportamiento` no aplica, con su motivo.
- Unitario: `modulo_borrado` elige un módulo tocado por el cambio.
- Unitario: sin rangos, los dos defectos eligen como hoy, con la misma semilla.
- Unitario: el caso guarda `en_cambio: true` cuando la línea cae dentro del cambio.
- Guarda: un caso viejo sin el campo se lee con `false`.

### 2. Suite que no compila

1. **Solo con `runner = "nextest"`.** Si la suite sale con 101, la suite no compila.
2. **Resultado.** Cada test esperado cuenta como fallo. El junit no se lee. El motivo es "la suite no compila".
3. **Veredicto.** Funciona y Completitud quedan con veredicto, y en rojo.
4. **Otros códigos** siguen con el cruce de hoy.
5. **Con otro runner, nada cambia.** `cargo test` usa 101 tanto para un test que falla como para un build roto.
6. **Nunca un verde.** La regla solo convierte un "sin veredicto" en rojo.

**Tests en rojo primero:**
- Unitario: con nextest, código 101 y un junit sin fallos, los tests esperados cuentan como fallos, con el motivo "la suite no compila".
- Unitario: con nextest, código 101 y sin junit, el resultado es el mismo. No es infraestructura.
- Unitario: con pytest, el código y el junit siguen el cruce de hoy.
- Unitario: con nextest y código 100 con fallos en el junit, el resultado es el de hoy.
- Guarda: con nextest y código 0 con el junit verde, el resultado es verde, como hoy.

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

Al terminar, escribí `docs/builds/informe-local-95.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Códigos de nextest**: los valores y su fuente.
3. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
