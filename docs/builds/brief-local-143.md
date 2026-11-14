# Brief de la Tarea 143 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-138`. Trae las Tareas 84 a 138 y la 127b.
- **Arrancá cuando los builders 139, 140 y 141 terminen.** El orquestador lo dice. Así no se carga la laptop.
- La 139 a la 142 tocan `src/oracle/contenedor.rs`, `src/oracle/descarga.rs`, `src/oracle/calibra/target.rs`, `tools/colab/`, `tests/colab_preparar.rs`, `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs`, `src/oracle/colab.rs`, `src/proveedores/mod.rs`, `src/main.rs` y `tests/claves_fd.rs`. No toques esos archivos.
- La 144 va después de esta tarea, sobre su punta.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-143`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el test de punta a punta y la causa | `src/oracle/calibra.rs`, `src/oracle/mod.rs`, `src/oracle/calibra/sonda_lastre.rs` |
| 2, la sonda en el repo con los tests comunes | `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs`, `src/oracle/eficiencia.rs` (solo para dar el filtro de los tests comunes) |
| 3, sin respaldo silencioso, y el estado guardado | `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra.rs`, `docs/interfaces.md` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **`src/oracle/eficiencia.rs`:** solo podés sumar una función que da el filtro de los tests comunes de la última medición. La 144 cambia el resto del archivo.
- **No toques:** los archivos de la 139 a la 142, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La Tarea 137 sumó una sonda de ejecución para elegir el lugar de `LastreDeMemoria`.** En la calibración de Eficiencia de t1 a t3 con la cadena 110 a 138, pasaron dos cosas.

**1. La sonda corrió, pero el inyector no usó su lista.**
- En el journal del orquestador, cada base muestra la sonda: 34 tests pasan en t1, y después `ROOT_SONDA_INICIO`, una lista de 101 funciones y `ROOT_SONDA_FIN`.
- Con la misma semilla, los seis lastres cayeron en los mismos lugares que con la regla de la Tarea 136. En t2 y t3, esos archivos no están en la lista de la sonda.
- `lastre_de_memoria` (`src/oracle/calibra/inyectar.rs:1298`) solo usa la regla de la 136 si `corridas` viene vacía. Con `corridas` llena, elige solo de la lista, o no elige nada.
- **Entonces `corridas` llegó vacía.** `tomar_sonda_lastre()` (`src/oracle/calibra.rs:1790`) dio `None`, un error o una lista vacía. Una lista vacía sale si los ids del contenedor no coinciden con los del mapa.
- **El planner no encontró la causa leyendo.** El camino corre en un solo hilo: `con_sonda_lastre` en la base, `correr_sonda_de_lastre` en `evaluar_con`, `dejar_sonda_lastre`, `tomar_sonda_lastre` y `corridas_de_la_sonda`.
- **El camino real nunca se probó de punta a punta.** El test `sonda_lastre_resultado_de_la_base_llega_al_inyector` (`src/oracle/calibra.rs:4065`) usa un evaluador falso que llama a `dejar_sonda_lastre` él mismo. No pasa por `evaluar_con`.
- **No hubo aviso.** Un error de la sonda deja un aviso en el veredicto de la base, que no se guarda. `None` o una lista vacía no dejan nada: `corridas_de_la_sonda` los convierte en una lista vacía.

**2. Aunque la lista hubiera llegado, la sonda mira el lugar equivocado.**
- **La sonda corre los tests de `bootstrap/` sobre el código de `bootstrap/`.** En t1 son 34 tests.
- **La medición de Eficiencia corre los tests comunes sobre el código del repo.** Son los que pasan en los dos lados.
- Un test viejo puede no ser común, porque el cambio lo rompe o lo borra. Una función que corre en `bootstrap/` puede no correr en el repo, porque el cambio deja de llamarla.
- En t1, los dos lastres cayeron en funciones `new` que la sonda vio correr en `bootstrap/`. La memoria pico no cambió: 0,993 y 1,001.
- En la vuelta (49), en t2, un lastre en `content_search/mod.rs` subió la memoria pico 2,4 veces. Ahí cayó en una función que los tests comunes corren.

## Tarea

La regla que manda: **la sonda ve lo mismo que la medición va a correr, y si no da una lista, el lastre no cae en un lugar adivinado.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El test de punta a punta, y la causa

1. **Un test nuevo** corre una calibración de Eficiencia con el evaluador real (`oracle::evaluar`), una fábrica falsa de herramientas, y un repo y un `bootstrap/` de prueba. Usá la fábrica falsa de los tests de `mod.rs`.
2. **La fábrica falsa** responde al comando de la sonda con la salida de verdad: los tests, la marca de inicio, los ids de las funciones que recibió marcadas, y la marca de fin. Los ids los saca de la copia marcada que recibe.
3. **El test afirma** que el lastre del primer caso cae en una función de esa lista.
4. **Con el test en rojo, buscá la causa.** Anotá en el informe dónde se perdía la lista, con la línea.
5. **Arreglá la causa.** El test queda en verde.

**Tests en rojo primero:**
- El test de este punto, en rojo con el código de hoy. Si da verde, anotalo: entonces la causa está fuera del camino que recorre, y el punto 3 la cubre igual.

### 2. La sonda en el repo, con los tests comunes

1. **`eficiencia.rs` suma una función** que da el filtro de los tests comunes de la última medición del hilo, como `tomar_referencia()`. Es el mismo filtro que arma la medición para `tiempo_tests` y `memoria_pico`.
2. **La sonda corre después de la medición de Eficiencia de la base,** en `evaluar_con`, cuando el valor de hilo de `con_sonda_lastre` está fijado.
3. **La sonda marca las funciones de producción del repo bueno,** no las de `bootstrap/`. Las reglas de qué se marca no cambian: nada `const`, nada en `unsafe`, nada de tests, ningún `main` de ejemplo.
4. **La sonda corre solo los tests comunes,** con ese filtro. Sin filtro, o con el filtro vacío, la sonda da un error con su motivo.
5. **El resultado** son las funciones del repo que los tests comunes corrieron, con su archivo y su nombre en el repo.
6. **La regla de la 134,** que pide una función que ya existía en `bootstrap/`, deja de aplicarse cuando hay lista de la sonda. La lista ya dice qué corre en el repo.

**Tests en rojo primero:**
- Unitario: con la sonda pedida, la copia marcada sale del repo bueno, no de `bootstrap/`.
- Unitario: el comando de la sonda lleva el filtro de los tests comunes.
- Unitario: sin filtro de tests comunes, la sonda da un error con su motivo.
- Unitario: una función nueva del repo que corre en los tests comunes queda en la lista.

### 3. Sin respaldo silencioso, y el estado guardado

1. **Si la calibración pidió la sonda,** y la sonda no dio funciones (`None`, error o lista vacía), `LastreDeMemoria` queda no aplicable, con el motivo de la sonda. No usa la regla de la 136.
2. **La regla de la 136 sigue** solo cuando la sonda no se pidió.
3. **`Fila` suma `sonda_lastre`,** con `#[serde(default)]`: si se pidió, si corrió, cuántas funciones dio, y su motivo si falló.
4. **Cada renglón de caso con `LastreDeMemoria` suma `lugar_por`,** con `#[serde(default)]`: `sonda` o `nombre`.
5. **Un aviso de la sonda** llega a los avisos de la calibración, además del veredicto de la base. Así `calibrar` lo imprime.
6. Una fila vieja sin los campos se lee. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: con la sonda pedida y una lista vacía, el caso de `LastreDeMemoria` queda no aplicable, con el motivo. Hoy usa la regla de la 136.
- Unitario: la fila guarda `sonda_lastre` con el número de funciones.
- Unitario: el renglón del caso guarda `lugar_por: sonda`.
- Unitario: un error de la sonda llega a los avisos de la calibración.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La sonda real sobre el repo de s2-t1,** con los tests comunes. Cuántas funciones da, y si incluye las de `content_search/mod.rs` en s2-t2.
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 143 y la 144, en una ventana sin otro trabajo en la laptop. Los lastres deben subir la memoria pico.

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

Al terminar, escribí `docs/builds/informe-local-143.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La causa**: dónde se perdía la lista de la sonda, con el archivo y la línea, o por qué el test de punta a punta no la mostró.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
