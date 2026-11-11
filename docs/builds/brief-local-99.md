# Brief de la Tarea 99 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-97` (7af9a54). Las Tareas 84 a 96 todavía no están mezcladas.
- Las Tareas 97 y 98 se construyen a la vez, en otras ramas. No tocan tus archivos de código.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-99`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario. Se mezcla después de la 97 y la 98.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la regla de decisión | `src/oracle/informe.rs`, `src/tipos.rs`, `docs/interfaces.md` |
| 2, `Suite::NoCompila` | `src/oracle/mod.rs` |

- **Orden:** primero el punto 1, que define el campo `no_compila`. Después el punto 2, que lo llena.
- **`docs/interfaces.md`** también lo cambia la Tarea 97. Es un archivo generado: regeneralo en tu rama, y se regenera otra vez al mezclar.
- **No toques:** `src/oracle/mutacion.rs`, `src/oracle/calibra.rs` y `src/oracle/calibra/inyectar.rs`, que son de la 97 y la 98. Tampoco el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Un producto que no compila da "sin decisión".** El orquestador evaluó, sin API y con el binario de la 96, una copia de t1 de S2 con un error de sintaxis. El detalle está en `/home/mpera/.cache/root-wt/orquestador/evaluacion-real-96.md`.
- nextest salió con 101, y el motivo fue el correcto: "la suite no compila".
- Pero las nueve dimensiones dieron "suite del contrato sin veredicto: la suite no compila". El estado fue sin veredicto.

**Dónde corta.** En `src/oracle/mod.rs`, cerca de la línea 695, la suite del contrato corre primero. Si da `Suite::SinVeredicto` (`enum Suite`, línea 387), Root arma un veredicto sin veredicto en todas las dimensiones y termina. La regla de la Tarea 95 marca en rojo los tests esperados con 101. En la suite del contrato de S2, los requisitos no nombran tests `contrato:`. Sin esperados, queda sin veredicto, y corta todo.

## Tarea

La regla que manda: **un producto que no compila está "no listo", y nunca "listo".** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La regla de decisión

1. `Veredicto` (`src/tipos.rs`) suma `no_compila: bool`, con `#[serde(default)]`.
2. `no_listo` (`src/oracle/informe.rs:413`) suma un disparo: si `no_compila` es verdadero, la regla es "el producto no compila".
3. Va antes de los demás disparos de `no_listo`.
4. Los bloqueos siguen antes que `no_listo`, como hoy. Un producto que no compila y tiene un secreto da "bloqueado".
5. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un veredicto con `no_compila: true` y sin bloqueos da "no listo", con la regla "el producto no compila".
- Unitario: con `no_compila: true` y un secreto, da "bloqueado".
- Guarda: un veredicto viejo sin el campo se lee con `false`, y su decisión no cambia.

### 2. `Suite::NoCompila`

1. `Suite` suma la variante `NoCompila`. Solo con `runner = "nextest"`: si la suite del contrato sale con 101, el resultado es `NoCompila`, no `SinVeredicto`.
2. **El oculto no corre.** Usa el mismo build. Cada test oculto esperado cuenta como fallo, con el motivo "el producto no compila".
3. **Dimensiones que necesitan compilar:**
   - Funciona da 0, con veredicto.
   - Completitud sale de los ocultos en rojo, y da 0 con veredicto.
   - Pruebas queda sin veredicto, porque cargo-mutants no compila. Revisá que hoy ya pase así, sin tocar `mutacion.rs`.
4. **Sondas que no necesitan compilar** siguen como hoy: gitleaks, cargo audit, y las sondas nativas de Estructura y Arquitectura. Una sonda que necesita compilar, como clippy, deja su ítem sin veredicto.
5. **El juez no corre.** `juzgar_diseno` (`mod.rs:1184`) no se llama. El juicio queda vacío, con el aviso "sin juez: el producto no compila".
6. El veredicto lleva `no_compila: true`.
7. Si la suite del contrato da 101 con otro runner, nada cambia.
8. Si el oculto da 101 y el contrato compiló, sigue la regla de la Tarea 95, como hoy.

**Tests en rojo primero:**
- Unitario: con un `correr` falso, el contrato sale con 101 bajo nextest. El veredicto trae Funciona en 0 con veredicto, los ocultos esperados en fallo y `no_compila: true`.
- Unitario: en ese caso, el `correr` falso no recibe la suite oculta, y el juez falso no recibe ninguna llamada.
- Unitario: en ese caso, las sondas de Seguridad siguen y dan su resultado.
- Guarda: con pytest y código 1 en el contrato, el resultado es el de hoy.
- Guarda: con nextest y contrato en 0, la evaluación sigue como hoy.

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

Al terminar, escribí `docs/builds/informe-local-99.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Dimensiones con un producto que no compila**: qué da cada una de las nueve.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
