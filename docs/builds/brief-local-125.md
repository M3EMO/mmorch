# Brief de la Tarea 125 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-122`. Trae las Tareas 84 a 122.
- La 123 y la 124 se construyen a la vez. La 123 toca `src/oracle/mutacion.rs` y `src/oracle/calibra.rs`. La 124 toca `src/lib.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-125`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la coma final | `src/oracle/juez.rs` |
| 2, el reintento por JSON inválido | `src/oracle/juez.rs` |

- Los dos puntos tocan el mismo archivo. Van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra.rs`, `src/oracle/mutacion.rs`, `src/lib.rs`, `src/proveedores/`, `src/ficha/juez.rs`, `src/oracle/auditar.rs`, `src/oracle/descarga/juez_config.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`. No sumes un parser de JSON tolerante.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La medición P101 a de Diseño de s2-t1 dio n=0 por una coma.** La base se juzgó tres veces. Los juicios 1 y 2 dieron niveles para los seis ítems, y varían como mucho 1 nivel. El juicio 3 quedó sin veredicto: "JSON inválido: trailing comma at line 11 column 9". La calibración no midió casos. Costó 0,2276 USD.

**Lo que hace hoy `src/oracle/juez.rs`.**
- `pedir` (línea 706) llama al modelo y después a `parsear` (línea 1099).
- `parsear` usa `serde_json::from_str`, que es estricto. Una coma final antes de `}` o `]` es un error.
- **Un JSON inválido no tiene reintento.** `juzgar` (línea 717) devuelve sin veredicto enseguida, cuando `pedir` falla.
- El reintento de hoy (`reintentar`, línea 808) solo pide de nuevo los ítems rotos o frenados por el gate, en una respuesta que ya se leyó.

**Por qué reparar solo la coma final.** Un objeto con coma final dice lo mismo que sin ella. La reparación no cambia ningún nivel, cita ni hallazgo. Otros errores, como un texto cortado o una comilla que falta, sí pierden contenido. Adivinar el contenido de un juez es peor que no tener juicio.

**La huella no debe cambiar.** `huella_del_prompt` (`src/oracle/juez.rs:71`) suma `PROMPT_SISTEMA`, `MARCA_RECORTE` y `VERSION_LECTURA`. Las filas de calibración guardan esa huella. Una fila de Arquitectura de s2-t1 recién medida costó 1,376 USD. Si la huella cambia, esa fila deja de valer.

## Tarea

La regla que manda: **una coma final no tira un juicio, y un JSON roto tiene un reintento.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La coma final

1. **Una función pura** quita las comas finales: una coma seguida solo de espacios, tabuladores o saltos de línea, y después `}` o `]`.
2. **Respeta los textos entre comillas.** Una coma dentro de un texto JSON no se toca, aunque la siga `}`. Respeta también las comillas escapadas con `\"`.
3. **`parsear` prueba primero el parseo estricto.** Solo si falla, prueba con el texto reparado.
4. **Si el texto reparado parsea,** el juicio sigue, y suma un descarte "JSON reparado: coma final". Elegí el lugar del descarte para que llegue al veredicto como los demás.
5. **Si el texto reparado tampoco parsea,** el error es el del parseo estricto, como hoy.
6. **Ninguna otra reparación.**

**Tests en rojo primero:**
- Unitario: `{"items":[{"item":"a","nivel":1,}],"hallazgos":[],}` parsea, con un nivel. Hoy da "JSON inválido".
- Unitario: una coma final dentro de un bloque ```json también se repara.
- Unitario: el texto `{"items":[{"item":"a","cita":"x,}"}],"hallazgos":[]}` no cambia: la coma está dentro de un texto.
- Unitario: una comilla escapada antes de la coma no confunde a la función: `"a\",}"` sigue igual.
- Unitario: un JSON cortado sigue dando "JSON inválido".
- Unitario: un juicio reparado trae el descarte "JSON reparado: coma final".

### 2. El reintento por JSON inválido

1. **Si `parsear` falla después de la reparación,** `juzgar` reintenta la llamada entera una vez.
2. **El prompt del reintento** es el mismo, más una línea fija: la respuesta anterior no era JSON válido, y debe responder solo el objeto. Esa línea va en una constante nueva.
3. **La constante nueva no entra a `huella_del_prompt`.**
4. **El reintento lleva `reintento_juez` en verdadero,** como el reintento de hoy. Así pasa por el mismo control de gasto.
5. **Si el reintento también falla,** el juicio queda sin veredicto. El motivo nombra los dos errores.
6. **Un error que no es de JSON,** como un modelo que falla o una respuesta truncada, no se reintenta aquí. Sigue como hoy.
7. **Después de un reintento por JSON,** el reintento de ítems rotos de hoy sigue igual, sobre la respuesta nueva.

**Tests en rojo primero, con un modelo falso:**
- Unitario: un modelo que da primero un JSON cortado y después uno válido da un juicio con niveles. El modelo se llama dos veces. Hoy da sin veredicto.
- Unitario: el segundo prompt trae la línea del reintento, y lleva `reintento_juez` en verdadero.
- Unitario: un modelo que da dos JSON cortados da sin veredicto, con los dos errores en el motivo. El modelo se llama dos veces, no tres.
- Unitario: un modelo que falla con error de proveedor se llama una sola vez.
- **Guarda de la huella:** `huella_del_prompt()` y `calibra::huella_de` de Diseño y de Arquitectura dan el mismo valor antes y después de la tarea. Anotá los valores en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Los tests del juez con `saltar_sin_podman`, si los hay.
- **La repetición de P101 a para Diseño de s2-t1,** con juez y API, con el binario de la cadena con la 125. Cuesta entre 1,2 y 1,4 USD. La pide el planner aparte.

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

Al terminar, escribí `docs/builds/informe-local-125.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La huella**: los valores de `huella_del_prompt` y de `huella_de` de Diseño y Arquitectura, antes y después.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
