# Brief de la Tarea 131 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-129`. Trae las Tareas 84 a 129 y la 127b.
- La 130 se construye a la vez. Toca solo `src/oracle/memo.rs`. No toques ese archivo.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-131`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el defecto `LastreDeMemoria` | `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |
| 2, la referencia reusada | `src/oracle/eficiencia.rs`, `src/oracle/calibra.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia, como la cantidad de defectos de `TODOS`. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/memo.rs`, `src/oracle/mod.rs`, `src/oracle/perfiles/herramientas.rs`, `src/tipos.rs`, `src/lib.rs`, `rubricas/`, el resto de `src/`, `imagenes/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.** Si un punto lo pide, detenete y escribilo en el informe.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Eficiencia no se puede calibrar en s2-t1.** Su fila tiene un `tarea_hash` viejo, y rehacerla no alcanza. La decisión P102 a del usuario pide, para una dimensión de oráculo, un defecto detectado y todos los limpios bien. Hoy no hay un defecto que la medición automática de la Tarea 115 vea:
- El único defecto de Eficiencia es `BucleCuadratico`, sobre `eficiencia.tiempo`. Necesita el camino caliente de `[eficiencia]`, y s2-t1 no trae esa sección.

**Por qué un defecto de memoria, y no de tiempo.**
- La medición automática compara el pico de memoria de los tests comunes contra `bootstrap/`. En s2-t1, es de 42 MB en los dos lados.
- Una reserva grande que se suelta al salir de la función sube el pico una vez. No depende de cuántas veces se llama la función.
- Un retardo de tiempo sí depende de las llamadas. Puede llevar un test a su plazo.
- Con 64 MB más, la razón es cercana a 2,5. Pasa el 1,5 de la decisión P105 a.
- `vec![0u8; n]` puede pedir páginas sin escribirlas, y no sube la memoria residente. `vec![1u8; n]` las escribe.
- El ítem de memoria de la rúbrica es `eficiencia.capacidad` (`rubricas/eficiencia.toml`).

**Por qué reusar la referencia.** La evaluación con Eficiencia de s2-t1 tardó unos 16 minutos. Cada caso de calibración repite toda la medición, con los dos lados. El lado `bootstrap/` es igual en todos los casos de una calibración. Medirlo una vez en la base ahorra la mitad de la medición de cada caso.

**Lo que se pierde.** Los casos ya no alternan con la referencia. Una carga de la máquina entre la base y un caso no pega en los dos lados. En la calibración, los casos corren seguidos, y el ruido de la referencia sigue saliendo de sus tres corridas. El planner lo acepta en la calibración. Una evaluación normal sigue alternando.

## Tarea

La regla que manda: **Eficiencia tiene un defecto que la medición automática ve, y la calibración mide la referencia una sola vez.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El defecto `LastreDeMemoria`

1. **El defecto suma una línea** al principio del cuerpo de una función de producción: `let _lastre = std::hint::black_box(vec![1u8; 64 << 20]);`.
2. **El tamaño** va en una constante, con su motivo en el comentario: 64 MB más sobre los 42 MB de s2-t1 da una razón cercana a 2,5.
3. **El lugar:** una función con cuerpo en un archivo del cambio, si hay `bootstrap/`. Sin `bootstrap/`, cualquier función de producción. Seguí la forma de `EstadoGlobal` de la Tarea 129.
4. **No vale** en una función `const`, en un bloque `unsafe`, en código de tests ni en un `main` de ejemplo.
5. **`Defecto::LastreDeMemoria`** se suma a `Defecto` y a `TODOS`. Su dimensión es Eficiencia, y su ítem es `eficiencia.capacidad`.
6. **`BucleCuadratico` no cambia.** Sin `[eficiencia]`, sigue sin poder aplicarse.

**Tests en rojo primero:**
- Unitario: el inyector deja la línea en una función del cambio de un repo de prueba.
- Unitario: el repo inyectado compila con `cargo check --offline`, como los tests de `inyectar.rs` que ya lo hacen.
- Unitario: `de_dimension(Dimension::Eficiencia)` trae `BucleCuadratico` y `LastreDeMemoria`.
- Guarda: los tests de `BucleCuadratico` siguen verdes.

### 2. La referencia reusada

1. **Un valor de hilo nuevo en `eficiencia.rs`** guarda las corridas del lado `bootstrap/`: tiempo, memoria, compilación y tamaño. Sigue la forma de `con_plazo_fijo` de `mutacion.rs`.
2. **La base de la calibración** mide los dos lados, como hoy, y guarda la serie de la referencia. Una función `tomar_referencia()` la devuelve, como `tomar_muestra()` de la mutación.
3. **Los casos** corren dentro de `con_referencia(serie, f)`. Con ese valor, `medir` no llama al ejecutor del lado inicial. Usa la serie guardada, y mide solo el lado del repo.
4. **Sin ese valor,** `medir` mide los dos lados y alterna, como hoy. Una evaluación normal no cambia.
5. **Una medida que la base no tomó** queda sin valor en el caso, con el motivo de la base.
6. **`calibra.rs`** toma la serie después de la base, y pasa el valor a los casos, junto a `con_plazo_fijo`.

**Tests en rojo primero, con un ejecutor falso por lado que cuenta sus llamadas:**
- Unitario: con la referencia fijada, el ejecutor del lado inicial no recibe ninguna llamada. Hoy recibe las de las tres corridas.
- Unitario: con la referencia fijada, la razón usa la serie guardada.
- Unitario: sin la referencia fijada, el orden de las llamadas alterna los dos lados, como hoy.
- Unitario: en una calibración de Eficiencia con un evaluador falso, la base mide los dos lados y cada caso mide uno.
- Guarda: los tests de `eficiencia.rs` de la 115, la 119 y la 120 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Que `LastreDeMemoria` compile y pase los tests de s2-t1 dentro de la imagen.
- Que los tests comunes de s2-t1 llamen a la función elegida, y que `memory.peak` suba.
- **La calibración de Eficiencia de s2-t1,** con n=4 y sin API, con el binario de la cadena con la 131. Anota la razón de memoria de cada caso, y el tiempo total.

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

Al terminar, escribí `docs/builds/informe-local-131.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El defecto nuevo**: un ejemplo del texto que inyecta, con su archivo y su línea en el repo de prueba.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
