# Brief de la Tarea 154 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (64) y (65).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-153`, cuando la 153 termine. Las dos tocan `src/colab_calibra.rs`.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-154`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las cifras de S2 con nombre | `src/colab_calibra.rs` |
| 2, el gasto desde el corte | `src/colab_calibra.rs` |
| 3, las cifras a la vista | `src/colab_calibra.rs` |

- Los tres puntos comparten archivo. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/observe/`, `src/lib.rs`, `src/cli.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila`, `Veredicto` ni `Evento`. El manifiesto del paquete sí suma campos. Si `r00t esquema` o `docs/interfaces.md` lo listan, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`. El crate `time` ya es dependencia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No leas `~/.local/share/root/eventos.jsonl`.** Los tests usan eventos falsos.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**El tope de S2 queda hoy en 0.** `gastado_s2_usd` llama a `observe::uso::gasto_api_desde(&eventos, 0.0)`. Suma todo el gasto de API de `eventos.jsonl`, de cualquier época. En la laptop, eso pasa de 200 USD. Con un tope de 25 USD, no queda nada, y P107 a no puede correr.

**Los eventos no dicen a qué tarea pertenecen.** Todas las llamadas del juez usan la corrida `oraculo-juez-diseno`. La fecha tampoco separa S2 de lo demás.

**El usuario decidió P112 a, literal: "a) 6,55 más lo de hoy".** Cuentan contra los 25 USD:
- los 6,55 USD que sumó el orquestador, corrida por corrida;
- más toda llamada a la API desde el 2026-11-14 a las 00:00 UTC, de cualquier tarea.

**Las dos cifras son decisiones del usuario.** No salen de una medida del código. Por eso deben tener nombre, fuente y estar a la vista. Un número suelto dentro de una función es difícil de encontrar y de cambiar.

## Tarea

La regla que manda: **cada cifra del tope tiene nombre y fuente, y cada paquete muestra cómo se calculó su tope.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Las cifras de S2 con nombre

1. **Al lado de `TOPE_S2_USD`, sumá dos constantes:**
   - `GASTO_S2_PREVIO_USD: f64 = 6.55`. Su comentario dice: suma del orquestador, corrida por corrida, antes del corte. Fuente: P112 a del usuario.
   - `CORTE_S2: &str = "2026-11-14T00:00:00Z"`. Su comentario dice: desde aquí, toda llamada a la API cuenta contra S2. Fuente: P112 a del usuario.
2. **Ningún epoch se escribe a mano.** Una función convierte `CORTE_S2` a segundos con el crate `time`. Un texto inválido da un error, no un pánico.
3. **Las tres constantes van juntas,** en un solo bloque, con un comentario que las presenta.

**Tests en rojo primero:**
- Unitario: `CORTE_S2` convertido da 1794614400. Hoy la constante no existe.
- Unitario: un texto de corte inválido da un error con el texto en el mensaje.

### 2. El gasto desde el corte

1. **`gastado_s2_usd` da `GASTO_S2_PREVIO_USD` más `gasto_api_desde(&eventos, corte)`.** No cambies `gasto_api_desde`.
2. **Su comentario dice la regla de P112 a,** y que sobreestima: cuenta las llamadas de cualquier tarea desde el corte.
3. **Las líneas de gasto que vuelven de Colab** cuentan si su `ts` es posterior al corte. La importación de la 142 conserva el `ts`, así que no hace falta otro cambio. Si no lo conserva, anotalo.

**Tests en rojo primero:**
- Unitario: con una llamada de 100 USD antes del corte y otra de 1 USD después, el gastado es 7,55. Hoy da 101.
- Unitario: una llamada después del corte con otra corrida, como `corre-memora-x`, también cuenta.
- Unitario: una reserva abierta después del corte cuenta. Una reserva cerrada por su llamada no se cuenta dos veces.
- Guarda: sin eventos, el gastado es 6,55, y quedan 18,45.

### 3. Las cifras a la vista

1. **`colab empaquetar --juez` imprime una línea de gasto,** antes de `ROOT_SHA_PAQUETE`:
   `gasto de S2: 6,55 USD previos + X USD desde 2026-11-14T00:00:00Z = Y USD; quedan Z USD de 25`.
   Las cifras salen de las constantes y del cálculo, nunca de un literal en el texto.
2. **El manifiesto guarda las cuatro cifras:** el previo, el corte, el gasto desde el corte y lo que queda. Junto al tope que ya guarda la 142.
3. **Sin `--juez`, nada cambia.** El paquete no lleva esas cifras, y la línea no sale.

**Tests en rojo primero:**
- Unitario: con eventos falsos, la línea de gasto trae el previo, el corte, el gasto, el total y lo que queda. Hoy no existe.
- Unitario: el manifiesto de un paquete con `--juez` trae las cuatro cifras.
- Guarda: un paquete sin `--juez` no trae esas cifras.
- Guarda: el rechazo de la 153 con el tope agotado sigue, ahora con el cálculo nuevo.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 153 y la 154.
- La línea de gasto de `empaquetar --juez` con el `eventos.jsonl` real. El orquestador la compara con su propia suma.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-154.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las cifras de S2**: dónde vive cada una, y por dónde sale a la vista.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
