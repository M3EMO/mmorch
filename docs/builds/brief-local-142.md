# Brief de la Tarea 142 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de la cadena que suma la 139, la 140 y la 141. El orquestador te da su nombre.
- **El punto 3 espera la Prueba 1 del orquestador.** Los puntos 1, 2 y 4 no la esperan. Si la Prueba 1 no terminó, hacé el 1, el 2 y el 4, y dejá el 3 para el final.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-142`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la clave por descriptor | `src/proveedores/mod.rs`, `src/main.rs`, `tests/claves_fd.rs` (nuevo) |
| 2, el juez en el paquete y el gasto de vuelta | `src/colab_calibra.rs`, `src/cli.rs` |
| 3, la subida al runtime del notebook | `src/colab_calibra.rs`, `tools/colab/notebook-juez.py` |
| 4, el guion del notebook | `tools/colab/notebook-juez.py` (nuevo) |

- El punto 1 y el 2 no comparten archivos. Corren en paralelo. El 4 va después del 1. El 3 va al final.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/lib.rs`, `src/tipos.rs`, `src/corre/`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila` ni de `Veredicto`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a la CLI `colab` real, a `colab-mcp`, ni a ninguna API.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, literal:** P109 "b) Notebook de Colab". Las calibraciones con juez (Diseño, Arquitectura y Estructura) corren en un runtime de Colab abierto desde un notebook en el navegador del usuario.

**La regla de las claves:** entran a Colab solo por Colab Secrets, y nunca se imprimen. `colab exec` de la CLI no lee Colab Secrets. Desde el kernel de un notebook abierto en el navegador, `google.colab.userdata.get` sí los lee, con la aprobación del usuario en pantalla.

**Lo que hoy hace Root con la clave.** `Claves::del_entorno` (`src/proveedores/mod.rs:208`) lee `DEEPSEEK_API_KEY` y `ANTHROPIC_API_KEY` del entorno. Si la clave está en el entorno de `r00t`, cada proceso que `r00t` lanza la hereda: `cp`, `tar`, `bwrap` y lo que corre adentro. La Tarea 139 hace que `bwrap` empiece con `--clearenv`, pero otros hijos de `r00t` heredan el entorno igual.

**El diseño: la clave viaja por un descriptor de archivo.** La celda del notebook abre una tubería, lanza `r00t` con la punta de lectura y sin claves en el entorno, y escribe la clave en la tubería. `r00t` la lee al arrancar, antes de lanzar ningún proceso, y cierra el descriptor. Ningún hijo hereda la clave ni el descriptor.

**Lo que la Tarea 141 no tiene, porque no lo necesitaba:**
- Rechaza las dimensiones con juez.
- No trae las líneas de gasto de vuelta.
- No pone el tope de S2 en el manifiesto.
- Sube el paquete con `colab upload`, a un runtime que abrió la CLI. El runtime del notebook lo abre el navegador.

## Tarea

La regla que manda: **una calibración con juez corre en el runtime del notebook, con la clave solo en la memoria de `r00t`, dentro del tope de S2, y su gasto vuelve a la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La clave por descriptor

1. **Una variable nueva, `ROOT_CLAVES_FD`.** Si existe, `r00t` lee del descriptor que nombra líneas `DEEPSEEK_API_KEY=<valor>` y `ANTHROPIC_API_KEY=<valor>`, hasta el fin del archivo. Otra clave, o una línea mal formada, da un error que no muestra el valor.
2. **La lectura ocurre en `main`, como primera acción,** antes de crear hilos o lanzar procesos. Después, `r00t` cierra el descriptor.
3. **Las claves leídas quedan en un `OnceLock`** de `proveedores`. `Claves::del_entorno` usa esas claves si están. Si no, lee el entorno, como hoy.
4. **`r00t` no pone las claves en su propio entorno.** No usa `set_var`.
5. **`ROOT_CLAVES_FD` sale del entorno** al terminar la lectura, para que ningún hijo la vea. Si quitarla del entorno pide `unsafe`, hacelo antes de crear hilos, con un comentario que lo diga.
6. **`Clave` sigue mostrándose como `Clave(***)`.** Ningún mensaje de error muestra un valor.

**Tests en rojo primero:**
- Unitario: una función pura lee `DEEPSEEK_API_KEY=sk-1\n` de un lector y da la clave. Hoy no existe.
- Unitario: una línea `OTRA=x` da un error, y el mensaje no contiene `x`.
- Unitario: con claves del descriptor, `Claves::del_entorno` las devuelve aunque el entorno no las tenga.
- **Integración, en `tests/claves_fd.rs`:** lanza el binario `r00t` con una tubería en `ROOT_CLAVES_FD` y un comando que lanza un hijo. El hijo imprime su entorno y sus descriptores abiertos. Ni la clave ni el descriptor aparecen. Si `r00t` no tiene un comando que lance un hijo simple, anotalo y probá con el más simple que exista.

### 2. El juez en el paquete, y el gasto de vuelta

1. **`r00t colab empaquetar --juez`** acepta las dimensiones con ítems del juez. Sin `--juez`, el rechazo de la 141 sigue igual.
2. **Eficiencia se sigue rechazando,** con o sin `--juez`.
3. **El manifiesto suma el tope de S2:** `25 − gastado`. El gastado sale de sumar en `eventos.jsonl` de la laptop las líneas de gasto de las tareas `s2-*`. El 25 va en una constante, con su motivo: la decisión P107 a del usuario.
4. **Cada trabajo con juez** corre con `--tope-usd` igual a lo que queda, según el manifiesto.
5. **El paquete de vuelta trae las líneas de gasto** de `eventos.jsonl` del runtime: las de tipo `llamada` y `reserva`.
6. **La importación las agrega** a `eventos.jsonl` de la laptop, con `"origen": "colab"` en su `extra`. No duplica una línea que ya está: misma corrida, misma reserva y mismo `ts`.
7. **El paquete nunca lleva una clave.** La lista de exclusiones de la 141 sigue.

**Tests en rojo primero:**
- Unitario: sin `--juez`, Diseño se rechaza. Con `--juez`, entra.
- Unitario: Eficiencia se rechaza con `--juez`.
- Unitario: con 6,55 USD de gasto en un `eventos.jsonl` falso, el tope del manifiesto es 18,45.
- Unitario: las líneas de gasto vuelven y se importan con `"origen": "colab"`, sin duplicarse.

### 3. La subida al runtime del notebook

**Espera la Prueba 1 del orquestador.** La Prueba 1 dice cuál de estas formas anda: `colab-mcp` sube el archivo, la CLI alcanza el runtime del notebook, o el notebook monta Google Drive.

1. **Implementá la forma que diga la Prueba 1,** y ninguna otra.
2. **Si es Drive,** el guion del punto 4 monta Drive y copia el paquete a `/content/paquete.tar`. El montaje pide su propia aprobación del usuario.
3. **En todos los casos,** el guion revisa el SHA-256 del paquete contra el manifiesto antes de usarlo.
4. **Si la Prueba 1 no terminó cuando llegues a este punto,** dejalo sin hacer y anotalo.

**Tests en rojo primero:**
- Los que pida la forma elegida, con falsos.

### 4. El guion del notebook

`tools/colab/notebook-juez.py`. El orquestador lo corre celda por celda con `colab-mcp`. Cada celda empieza con un comentario `# CELDA <n>: <qué hace>`.

1. **CELDA 1:** revisa que el paquete esté en `/content/paquete.tar`, verifica su SHA-256, lo abre, y corre `tools/colab/preparar-runtime.sh`. Imprime el resultado de la verificación.
2. **CELDA 2:** lee la clave con `userdata.get`. El usuario aprueba en pantalla. Imprime solo si la clave está.
3. **CELDA 3:** corre las etapas de P107 a en orden. Cada calibración lanza `r00t` con la clave por descriptor, como en el punto 1. Antes de cada etapa, aplica la regla de parada: si la sensibilidad con las tareas juntas de la etapa anterior da menos de 0,85, salta las etapas de esa dimensión. Imprime una línea por calibración: tarea, dimensión, sensibilidad, especificidad y gasto.
4. **CELDA 4:** arma el paquete de vuelta con las filas, los casos y las líneas de gasto. Imprime su ruta y su SHA-256.
5. **Las etapas y sus `n` vienen del manifiesto,** no del guion. El guion no decide nada de producto.
6. **El guion nunca imprime la clave,** ni la escribe en un archivo. La variable se borra con `del` después de entregarla.
7. **Una calibración que falla** no frena las demás de su etapa. La celda la anota y sigue.
8. **Una sesión cortada** se puede retomar: la CELDA 3 salta las calibraciones cuya fila ya está en el runtime.

**Tests en rojo primero, en `tests/claves_fd.rs` o con `python3 -m py_compile`:**
- `python3 -m py_compile tools/colab/notebook-juez.py` da 0.
- Un test corre la función de la regla de parada del guion con filas falsas: 0,80 salta la dimensión, y 0,90 la deja seguir.
- Un test corre la función de retomar: una calibración con fila ya guardada se salta.
- El guion se importa sin `google.colab`, para los tests: las importaciones de Colab van dentro de las funciones de las celdas.

No corras la validez, `corre`, `calibrar` ni la pasada reales. No llames a Colab ni a ninguna API. Los corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab, `colab-mcp` ni claves reales. Quedan para el orquestador:
- **La Prueba 1** de la vuelta (56), sección 3, antes del punto 3.
- **Una calibración real con juez en el notebook:** la etapa 1 de P107 a, dos casos de Diseño en t1, cerca de 0,35 USD. Después, la importación a la laptop, con el gasto sumado.
- **Las cinco etapas en una sola sesión del notebook,** con la presencia del usuario solo al empezar.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en el paquete, en un argumento, en un archivo ni en un log. Los tests usan valores falsos.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Idioma.** Los documentos y los mensajes del guion van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-142.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El camino de la clave**: desde `userdata.get` hasta la llamada a la API, paso por paso, y dónde no está.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
