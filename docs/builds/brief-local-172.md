# Brief de la Tarea 172 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (98).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde 5a4f295, en `claude/root-e7813e`.** Es el congelado Y.
- Trabajá en una rama nueva: `laptop/tarea-172`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- **La Tarea 171 puede correr a la vez, en otro runtime.** No comparten archivos. Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Por qué

**El usuario eligió correr D3 en Colab (P129 b).** D3 corre `r00t corre --tarea <tarea> --config ecc-haiku` sobre 31 tareas. La Tarea 171 da el builder con bwrap. Esta tarea da el paquete, el guion que corre las tareas en el runtime, y la importación de sus resultados a la laptop.

**Lo que esta tarea no hace: subir, lanzar, consultar ni bajar.** Eso lo hace un guion del orquestador, como en P107 a y juez-x. Así, ninguna clave ni token pasa por código de Root. `colab calibrar` ya descarta las claves del entorno a propósito, y esta tarea mantiene esa regla.

**Las corridas cortadas por Colab son comunes.** Se vieron un 408 al subir y un RemoteDisconnected al crear el runtime. Una corrida cortada no puede contar, y una tarea no puede contar dos veces.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el paquete de `corre` | un módulo nuevo `src/colab_corre.rs`, `src/cli.rs`, `src/lib.rs` |
| 2, el runtime de `corre` | dos guiones nuevos: `tools/colab/preparar-corre.sh` y `tools/colab/corre-runtime.py` |
| 3, la importación | `src/colab_corre.rs`, `src/cli.rs`, `src/lib.rs` |

- Los puntos van en serie: 1, 2 y 3.
- **`src/lib.rs` está en la lista a propósito.** Cablea los comandos nuevos.
- **Podés llamar** a funciones públicas de `src/colab_calibra.rs`, como la revisión del manifiesto o el SHA. No cambies ese archivo. Si hace falta cambiarlo, detenete y anotalo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test. Anotá cada uno en el informe.
- **No toques:** `src/corre/`, `src/oracle/`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/colab/red-builder.sh`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Ninguna huella cambia.**
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **cada tarea de una pasada cuenta una sola vez, con una corrida completa; nada secreto viaja en el paquete; y una corrida cortada por Colab no se pierde ni se cuenta.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El paquete de `corre`

1. **`r00t colab empaquetar --corre --tareas <t1,t2,…> --config <config> --pasada <N> --binario <r00t> --raiz-root <worktree> --salida <archivo>`.**
2. **El paquete trae:**
   - el binario de `--binario`, con su SHA-256;
   - de `--raiz-root`: `GOAL.md` y `tools/colab/`, porque `corre` necesita `GOAL.md` en su directorio de trabajo;
   - del `ROOT_HOME`: `root.toml`, `configs/<config>`, `calibracion.json` con sus casos, `eventos.jsonl`, y `oraculos/<tarea>/` de cada tarea pedida;
   - el plugin de ECC que nombra la config;
   - `versiones-builder.env`, con la versión de `claude` y de `rustc` de la imagen `root-builder:1`. Las lee `podman run` en la laptop;
   - un manifiesto con la pasada, el commit de `--raiz-root`, la lista de tareas en orden y el SHA de cada archivo.
3. **`eventos.jsonl` va** para que el ruteo de Colab vea el bloque de cupo vigente.
4. **El paquete no trae:** claves, tokens, `.credentials.json`, `.env`, ni otra tarea de `oraculos/`. El empaquetado revisa el paquete armado, y falla si encuentra un archivo con esos nombres o un texto con forma de clave, como `sk-`.
5. **Una tarea que no existe en el `ROOT_HOME`** da un error de uso, con su id.

**Tests en rojo primero:**
- Unitario: el paquete de 2 tareas trae `oraculos/` de esas 2, y de ninguna otra. Hoy `--corre` no existe.
- Unitario: un `ROOT_HOME` con un `.env` o un `.credentials.json` falso no los mete en el paquete.
- Unitario: un archivo con `sk-` falso dentro de una tarea frena el empaquetado, con su ruta.
- Unitario: el manifiesto trae la pasada, el commit y las tareas en orden.
- Guarda: `colab empaquetar` sin `--corre` arma el mismo paquete que hoy.

### 2. El runtime de `corre`

**`tools/colab/preparar-corre.sh <carpeta del paquete>`,** como root, en Colab:
1. Corre `preparar-runtime.sh`, como hoy para el oráculo.
2. Instala `nodejs`, `npm`, `slirp4netns`, `iptables` e `iproute2`, con reintentos de apt como hoy.
3. Instala `@anthropic-ai/claude-code` en la versión de `versiones-builder.env`. Revisa que `claude --version` y `rustc --version` coincidan con ese archivo. Si no coinciden, termina con un error que nombra las dos versiones.
4. Crea el usuario `builder`, sin carpeta propia y sin grupos extra.
5. Arma el `ROOT_HOME` en `/root/root-home`, con dueño root y modo 0700. Lo mismo vale para `/root`.
6. Corre `red-builder.sh`, de la Tarea 171. Si ese guion no está, termina con un error.

**`tools/colab/corre-runtime.py <carpeta del paquete>`,** lanzado aparte del exec, como `lanzar.py`:
1. **Toma la clave y el token del entorno, una sola vez,** y los saca de `os.environ`. La clave va solo al entorno de `r00t`. El token se escribe en `/run/root-token`, con dueño root y modo 0600, y su ruta va en `ROOT_TOKEN_BUILDER`. Nunca imprime ninguno de los dos.
2. **Para cada tarea del manifiesto, en orden:**
   - si `hechos/<tarea>.json` existe, la salta;
   - corre, desde la carpeta con `GOAL.md`: `r00t corre --tarea <tarea> --config <config>`, con `ROOT_HOME=/root/root-home`, `ROOT_CONTENEDOR_MODO=bwrap`, `ROOT_RED_BUILDER=root-builder` y `ROOT_USUARIO_BUILDER=builder`;
   - al terminar, arma `vuelta-<tarea>.tar.gz`: la fila nueva de `runs.jsonl`, sus eventos, sus transcripts, la salida de `r00t` y el código;
   - escribe `hechos/<tarea>.json` con el código, el SHA de la vuelta, el inicio y el fin. Lo escribe en un temporal y lo renombra.
3. **Escribe `estado.json` después de cada tarea,** en un temporal que renombra. Trae las tareas hechas, las que faltan, la que corre y su pid.
4. **Al terminar o al fallar,** borra `/run/root-token` y escribe `estado.json` con `fin` o `error`.
5. **Una tarea que tarda más de 7200 s** se corta. Su vuelta dice `cortada: plazo`.

**Tests en rojo primero, con Python y programas falsos:**
- Unitario: con 3 tareas y una ya en `hechos/`, el guion corre solo las otras 2. Hoy el guion no existe.
- Unitario: un `r00t` falso que sale con 1 deja su vuelta y su `hechos/`, y el guion sigue con la siguiente.
- Unitario: la clave y el token falsos no aparecen en `estado.json`, en las vueltas, ni en la salida del guion.
- Unitario: al terminar, `/run/root-token` ya no existe. En el test, la ruta es un temporal del worktree.
- Unitario: `preparar-corre.sh`, con comandos falsos, frena si `claude --version` no coincide.

### 3. La importación

1. **`r00t colab importar --corre <vuelta-tarea.tar.gz> --sha <sha> --ida <paquete>`.**
2. **Revisa, antes de escribir nada:**
   - el SHA de la vuelta;
   - que la tarea esté en el manifiesto del paquete de ida;
   - que el commit del paquete sea el `HEAD` de `--raiz-root`, como en `colab importar`.
3. **Clasifica la corrida:**
   - **completa:** `r00t` terminó con un veredicto, sin un `Gate` de cupo agotado, y sin un fallo de infraestructura del builder;
   - **cortada:** el plazo, un `Gate` de cupo agotado, o un fallo de infraestructura.
4. **El registro de la pasada,** `colab-corre.json` en `ROOT_HOME`, guarda por pasada y tarea: el SHA, la clase y el id de la corrida.
5. **Una tarea cuenta una sola vez por pasada:**
   - una vuelta completa de una tarea que ya tiene otra completa en esa pasada no entra: "ya hay una corrida completa";
   - una vuelta cortada entra como cortada, y no impide otra;
   - la misma vuelta dos veces no entra: "ya estaba".
6. **Lo que entra:** la fila de `runs.jsonl`, los transcripts, y los eventos con `origen` igual a `colab`. **Los eventos de gasto entran aunque la corrida esté cortada.** El gasto de S2 los cuenta.
7. **Imprime** la clase, el veredicto, el contrato y el oculto como pasa y total, los minutos y el gasto. Nunca imprime nombres de tests.

**Tests en rojo primero:**
- Unitario: una vuelta completa entra, y el registro la marca. Hoy `--corre` no existe.
- Unitario: una segunda vuelta completa de la misma tarea y pasada no entra.
- Unitario: una vuelta cortada y después una completa de la misma tarea entran las dos. Solo la completa cuenta.
- Unitario: los eventos de gasto de una vuelta cortada entran, y el gasto de S2 sube.
- Unitario: una vuelta con un SHA distinto no entra, y no escribe nada.
- Unitario: una tarea fuera del manifiesto no entra.
- Guarda: `colab importar` sin `--corre` hace lo mismo que hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/tarea-172`.
2. **`git diff 5a4f295 laptop/tarea-172 --stat`** toca solo los archivos de la tabla y el informe.
3. **Un paquete real de 1 tarea,** armado en la laptop. El orquestador lista sus archivos y busca `sk-`, `.env` y `.credentials.json`.
4. **La prueba de aislamiento en Colab,** de la sección 4 de la vuelta 98, cuando estén las dos tareas.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo del paquete. Los tests usan valores falsos.
- **Oráculo.** Ninguna salida lleva nombres de tests ni contenido de `oculto/`.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-172.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El paquete**: la lista de lo que entra y de lo que nunca entra.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
