# Brief de la Tarea 129 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-127`. Trae las Tareas 84 a 127.
- La 128 se construye a la vez. Toca `src/oracle/calibra.rs` y `src/oracle/mod.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-129`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, `EstadoGlobal` | `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |
| 2, `EfectoEnNucleo` | `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |

- Los dos puntos tocan los mismos archivos. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal que esta tarea cambia, como la cantidad de defectos de `TODOS`. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra.rs`, `src/oracle/mod.rs`, `src/oracle/juez.rs`, `src/tipos.rs`, `src/lib.rs`, `rubricas/`, el resto de `src/`, `imagenes/` y `$ROOT_HOME/`.
- **Si un defecto nuevo pide un cambio en `calibra.rs`,** detenete y escribilo en el informe. La 128 toca ese archivo.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Cada dimensión con juez tiene un solo defecto, y no prueba al juez.** Lo dice `Defecto::item` (`src/oracle/calibra/inyectar.rs:63`):
- Diseño tiene `ErrorTragado`, sobre `diseno.errores`.
- Arquitectura tiene `CicloEntreCapas`, sobre `arquitectura.dependencias`, que es un ítem de sonda.

**Lo que pasó en s2-t1:**
- `diseno.errores` vale 0 en la base. Los cinco defectos de Diseño cayeron en el piso. En Diseño, Arquitectura y Estructura, un ítem cuenta como bajado con 2 niveles. Desde 0 no puede bajar.
- El 1,0 de Arquitectura lo dio la sonda de ciclos. `arquitectura.adecuacion` e `arquitectura.interfaces`, que mide el juez, no tienen ningún defecto que los pruebe.

**Los ítems del juez con nivel 2 o más en la base de s2-t1:**
- `diseno.estado`: 3 en los tres juicios.
- `arquitectura.adecuacion`: 2, 3 y 3.

**El cambio de s2-t1** es `src/pattern/composite_pattern.rs`. El juez lee el cambio primero.

**Lo que ya existe para elegir el lugar.** Cada inyector recibe un `Prng` y los `Excluidos`, y devuelve un `Lugar`. Mirá `error_tragado` (`src/oracle/calibra/inyectar.rs:976`) como modelo. La Tarea 105 restringe los defectos de línea al cambio cuando hay `bootstrap/`.

## Tarea

La regla que manda: **cada dimensión con juez tiene un defecto para un ítem del juez, que compila, que no cambia el comportamiento, y que el juez puede ver.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. `EstadoGlobal`, para `diseno.estado`

1. **El defecto suma un contador global** a un archivo de producción: un `static` de tipo `std::sync::atomic::AtomicUsize`, con un nombre que no choque con otro del archivo.
2. **Una función del archivo lo suma y lo lee,** sin que el valor cambie lo que devuelve. Por ejemplo, `let _ = CONTADOR.fetch_add(1, Ordering::Relaxed);` al principio del cuerpo.
3. **El `use` necesario** va con ruta completa en la línea, para no tocar los `use` del archivo.
4. **El lugar:** una función con cuerpo en un archivo del cambio, si hay `bootstrap/`. Sin `bootstrap/`, cualquier función de producción.
5. **No vale** en una función `const`, en un bloque `unsafe` ni en código de tests.
6. **`Defecto::EstadoGlobal`** se suma a `Defecto` y a `TODOS`. Su dimensión es Diseño, y su ítem es `diseno.estado`.

**Tests en rojo primero:**
- Unitario: el inyector deja el `static` y la línea del contador en una función del cambio de un repo de prueba.
- Unitario: el repo inyectado compila con `cargo check --offline`, como los tests de `inyectar.rs` que ya lo hacen.
- Unitario: `de_dimension(Dimension::Diseno)` trae `ErrorTragado` y `EstadoGlobal`.
- Unitario, en `defectos.rs`: un juez falso que baja `diseno.estado` cuando ve el contador lo detecta, como `error_tragado_lo_detecta_el_juez_falso`.

### 2. `EfectoEnNucleo`, para `arquitectura.adecuacion`

1. **El defecto mete una lectura del entorno** dentro de una función del cambio: `let _ = std::env::var("ROOT_EFECTO");`. La lectura no cambia lo que la función devuelve.
2. **El nombre de la variable** es fijo y no existe en el entorno de los tests. Así la lectura nunca cambia el comportamiento.
3. **El lugar:** igual que el punto 1.
4. **No vale** en una función `const` ni en código de tests.
5. **`Defecto::EfectoEnNucleo`** se suma a `Defecto` y a `TODOS`. Su dimensión es Arquitectura, y su ítem es `arquitectura.adecuacion`.

**Tests en rojo primero:**
- Unitario: el inyector deja la lectura en una función del cambio de un repo de prueba.
- Unitario: el repo inyectado compila con `cargo check --offline`.
- Unitario: `de_dimension(Dimension::Arquitectura)` trae `CicloEntreCapas` y `EfectoEnNucleo`.
- Unitario, en `defectos.rs`: un juez falso que baja `arquitectura.adecuacion` cuando ve la lectura lo detecta.
- Guarda: los tests de `CicloEntreCapas` y `ErrorTragado` siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Que los dos defectos compilen y pasen los tests en t1, dentro de la imagen.
- **Que el juez real los vea.** Lo dice la repetición de P101 a para Diseño y Arquitectura, con juez y API. Espera la respuesta del usuario a P107.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Si el enum `Defecto` sale en `docs/interfaces.md`, regeneralo.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-129.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los defectos nuevos**: un ejemplo del texto que inyecta cada uno, con su archivo y su línea en el repo de prueba.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada `match` sobre `Defecto` que cambió, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
