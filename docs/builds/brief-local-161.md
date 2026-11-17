# Brief de la Tarea 161 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (77).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-160` (5b763f9). Trae las Tareas 84 a 160 y la 127b.
- **Arrancá cuando termine verif-160.** No corras mientras el orquestador use podman.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-161`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el trabajo lanzado aparte | `src/colab_calibra.rs` |
| 2, la consulta del estado | `src/colab_calibra.rs` |

- Los dos puntos comparten archivo. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/colab.rs`, `src/oracle/`, `src/lib.rs`, `src/cli.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.** Los tests usan el `ColabCli` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**`colab calibrar` corre cada trabajo en un solo exec largo.** `correr_runtime` llama a `ejecutar_guion` con `SEG_TRABAJO_CALIBRAR`, 14400 segundos, y espera su salida. `colab::paso` recorta ese plazo con lo que queda de `TOPE_RUNTIME`.

**Un exec largo ya colgó dos veces:**
- en la prueba 157b, el cliente de `colab exec` esperó 15,5 horas, por encima de su `--timeout`;
- en la prueba 157c, `calibrar` dio "colab superó 11072 s; su paquete de vuelta no llegó". El runtime seguía vivo, y el mismo trabajo, repetido, tardó 3043 segundos.

**La causa más probable:** el trabajo termina, y el cliente de `colab exec` no vuelve. Nadie guarda cuándo terminó el trabajo dentro del runtime, así que no se puede probar.

**El arreglo sirve para las dos explicaciones.** Si ningún exec dura más que unos segundos, un cliente colgado no corta nada. Si el trabajo de verdad tarda, un archivo de estado da su hora de fin. Además, el kernel del runtime queda libre entre consultas.

## Tarea

La regla que manda: **ningún exec del trabajo dura más que unos segundos, y el trabajo deja escrito cómo terminó.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El trabajo lanzado aparte

1. **`trabajo-N.py` corre el comando y, al terminar, escribe `trabajo-N.estado`** en la carpeta remota. Lleva el código de salida, y la hora de inicio y de fin, en una línea de JSON.
2. **El estado se escribe en un temporal y se renombra.** Nunca queda un estado a medias.
3. **Un guion nuevo, `lanzar-N.py`,** corre `trabajo-N.py` con `subprocess.Popen(..., start_new_session=True)`, con la salida a `trabajo-N.log`. Imprime `ROOT_LANZADO=<pid>` y sale enseguida.
4. **Root manda `lanzar-N.py` con un exec corto:** `--timeout 120` y un plazo local de 180 segundos.
5. **`script_trabajo` sigue pasando la guarda de secretos de la 158.**

**Tests en rojo primero:**
- Unitario: con el `ColabCli` falso, el exec del trabajo usa `--timeout 120`, no 14400. Hoy usa 14400.
- Unitario: el texto de `trabajo-N.py` escribe el estado en un temporal y lo renombra.
- Unitario: el texto de `lanzar-N.py` usa `start_new_session=True` e imprime `ROOT_LANZADO`.
- Guarda: un entorno con una variable secreta con valor sigue haciendo fallar `script_trabajo`.

### 2. La consulta del estado

1. **Root consulta cada 60 segundos** con un exec corto, `estado-N.py`, con `--timeout 120` y un plazo local de 180 segundos.
2. **`estado-N.py` imprime una de tres respuestas:**
   - `ROOT_ESTADO=fin` y el JSON del estado, si el archivo existe;
   - `ROOT_ESTADO=corriendo`, si el pid vive y no hay archivo;
   - `ROOT_ESTADO=murio` y las últimas 20 líneas de `trabajo-N.log`, si el pid no vive y no hay archivo.
3. **Con `fin`,** Root toma el código del estado y sigue como hoy: el cierre, la bajada y la importación.
4. **Con `murio`,** Root da "el trabajo terminó sin estado", con el final del log.
5. **Una consulta que no vuelve en su plazo** no corta nada. Root la cuenta y espera la siguiente. Con 5 fallas seguidas, da "Colab no responde", con la hora de la última respuesta.
6. **El plazo total del trabajo no cambia:** `SEG_TRABAJO_CALIBRAR`, recortado por `lim.restante()`. Al vencer, el error dice cuánto llevaba el trabajo, según su hora de inicio.
7. **El tiempo entre consultas** se pide a una función de espera que los tests pueden reemplazar. Los tests no duermen 60 segundos.
8. **Las señales siguen cortando la espera,** como hoy con `lim.cancelar`.

**Tests en rojo primero:**
- Unitario: con un `ColabCli` falso que responde `corriendo` dos veces y después `fin` con código 0, Root importa la vuelta. Hoy no existe la consulta.
- Unitario: con `murio`, el error trae el final del log.
- Unitario: con 5 consultas que no vuelven, el error es "Colab no responde". Con 4 fallas y después `fin`, todo sigue.
- Unitario: al vencer el plazo total, el error dice cuánto llevaba el trabajo.
- Guarda: con `--sin-stop`, las tres líneas `ROOT_` de la 157 salen igual.
- Guarda: sin `--sin-stop`, la lista de comandos termina en `stop`.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 161.
- Una corrida corta de `colab calibrar` en Colab, con `s2-t1:eficiencia:1`. Ahí se ve que el trabajo termina, que la vuelta llega y que el estado trae la hora de fin.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-161.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los guiones remotos**: un ejemplo de `lanzar-N.py`, `estado-N.py` y una línea de estado, con valores falsos.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
