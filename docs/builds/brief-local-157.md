# Brief de la Tarea 157 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (71).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-156`. Trae las Tareas 84 a 156 y la 127b.
- **Esta tarea depende de P118.** Si el usuario elige la opción b, el orquestador la descarta. Podés empezar antes de la respuesta.
- Ningún otro builder corre a la vez. No corras mientras el orquestador use podman.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-157`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la bandera `--sin-stop` | `src/cli.rs`, `src/lib.rs` |
| 2, el runtime que queda abierto | `src/colab_calibra.rs` |

- **`src/lib.rs` está en la lista a propósito.** Cablea las banderas de `ColabAccion` hacia `colab_calibra`. La 142 lo necesitó, y su brief lo había prohibido por error.
- Los dos puntos no comparten archivos. Corren en paralelo, pero el punto 1 necesita la firma del punto 2. Acordá la firma antes.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila`, `Veredicto` ni `CasoGuardado`. Si `activo.json` suma un campo, que sea opcional, con valor por defecto.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan las banderas de `colab calibrar`, regeneralos.
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

**La v6 de Eficiencia dio "1.00 núcleos de 2" de carga ajena en dos runtimes de tres.** El robo de CPU no lo explica: la Prueba 0g etapa 1 no lo mostró. Falta ver qué procesos gastan CPU durante un trabajo real.

**Un muestreo aparte no puede correr durante el trabajo.** El kernel del runtime corre los exec de a uno. Un segundo `colab exec` espera a que termine el trabajo de `colab calibrar`. Cuando corre, el runtime ya está quieto.

**La salida: un runtime preparado que queda abierto.** Con `--sin-stop`, `colab calibrar` hace todo como siempre, pero no llama a `stop` al final. El orquestador manda después un solo exec, que repite el trabajo en un subproceso y muestrea en paralelo. El kernel ve un solo exec, así que no hay espera. Ese guion es del orquestador, no de Root.

**El riesgo es un runtime olvidado.** Cada runtime de CPU cuesta cerca de 0,08 unidades por hora. Por eso el runtime abierto queda anotado, y el mensaje lo dice.

## Tarea

La regla que manda: **`--sin-stop` cambia una sola cosa: no se llama a `stop`. Todo lo demás corre igual, y el runtime abierto queda a la vista.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La bandera `--sin-stop`

1. **`ColabAccion::Calibrar` suma `sin_stop: bool`,** con `--sin-stop`. Por defecto vale falso.
2. **`lib.rs` la pasa a `colab_calibra`.** Sin la bandera, el comportamiento no cambia.
3. **La ayuda de la bandera dice el costo:** el runtime sigue abierto y gasta unidades hasta que alguien lo cierre.

**Tests en rojo primero:**
- Unitario: `r00t colab calibrar ... --sin-stop` llega a `colab_calibra` con `sin_stop` verdadero. Hoy la bandera no existe.
- Guarda: sin la bandera, llega falso.

### 2. El runtime que queda abierto

1. **Con `sin_stop`, `correr_runtime` no llama a `stop`.** La preparación, el trabajo, el cierre, la bajada y la importación corren igual.
2. **Por cada runtime abierto,** el comando imprime tres líneas:
   - `ROOT_SESION_ABIERTA=<sesión>`;
   - `ROOT_CARPETA_REMOTA=<carpeta>`, la carpeta del paquete en el runtime;
   - `ROOT_TRABAJO_CMD=<json>`: el comando del trabajo, con sus argumentos y los nombres de sus variables de entorno, en una línea de JSON.
3. **`ROOT_TRABAJO_CMD` nunca lleva un valor secreto.** Usá `es_nombre_secreto` para dejar fuera el valor de toda variable con nombre de clave. El nombre sí puede ir.
4. **`activo.json` conserva la sesión como hoy,** y la marca como abierta a propósito. Si eso pide un campo nuevo, que sea opcional, con valor por defecto.
5. **El aviso al final dice** que el runtime sigue abierto, y cómo cerrarlo: el comando de `colab stop` con la sesión, o `oraculo pendientes --liberar`.
6. **Un trabajo que falla con `sin_stop`** deja el runtime abierto igual, con las mismas líneas. Sirve para diagnosticar.
7. **Varios runtimes:** cada uno imprime sus líneas.
8. **El tope de `TOPE_RUNTIME` no cambia.** `--vigilar` y `--liberar` lo siguen aplicando.

**Tests en rojo primero:**
- Unitario: con el `ColabCli` falso y `sin_stop`, la lista de comandos no termina en `stop`. Hoy termina en `stop`.
- Unitario: con `sin_stop`, la salida trae `ROOT_SESION_ABIERTA`, `ROOT_CARPETA_REMOTA` y `ROOT_TRABAJO_CMD` por runtime.
- Unitario: una variable de entorno del trabajo con nombre de clave aparece en `ROOT_TRABAJO_CMD` sin su valor.
- Unitario: con `sin_stop` y un trabajo que falla, no se llama a `stop`, y las tres líneas salen igual.
- Unitario: con `sin_stop`, `activo.json` conserva la sesión después del comando.
- Guarda: sin `sin_stop`, la lista de comandos termina en `stop`, como hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 157.
- La Prueba 0g con `--sin-stop` en Colab, con el guion `repetir-0g.py` del orquestador.

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

Al terminar, escribí `docs/builds/informe-local-157.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las líneas impresas**: un ejemplo de las tres, con valores falsos.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
