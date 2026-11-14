# Brief de la Tarea 145 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-141`. Trae las Tareas 84 a 141 y la 127b.
- La 143 y la 144 corren a la vez, en serie entre ellas. Tocan `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/eficiencia.rs` y `docs/interfaces.md`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-145`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, los puntos de montaje en el runtime | `tools/colab/preparar-runtime.sh`, `tools/colab/verificar-versiones.sh`, `tests/colab_preparar.rs` |
| 2, los mensajes de `bwrap` | `src/oracle/contenedor.rs` (solo los textos de los mensajes) |
| 3, los repos enteros | `src/colab_calibra.rs` |
| 4, `n` y `semilla` por trabajo | `src/colab_calibra.rs`, `src/cli.rs` |
| 5, la vuelta armada en el runtime, e importar aunque falle el `stop` | `src/colab_calibra.rs` |

- Los puntos 1 y 2 no comparten archivos con los demás. Corren en paralelo. Los puntos 3, 4 y 5 tocan `colab_calibra.rs`, y van en serie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 143 y la 144, `src/lib.rs`, `src/oracle/colab.rs`, `src/tipos.rs`, `src/proveedores/`, `src/main.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **`docs/interfaces.md` no se regenera aquí.** La 143 lo cambia. Si el texto de los comandos aparece ahí, anotalo en el informe, y el orquestador lo regenera al encadenar.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a la CLI `colab` real ni a podman real.** Los tests usan el falso de `ROOT_COLAB_BIN` y el podman falso de la 141.
- **No corras `preparar-runtime.sh` en la laptop.** Instala paquetes del sistema.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Los informes de la 139, la 140 y la 141 dejaron cinco huecos.** El planner los decidió en la vuelta (58).

**1. Los puntos de montaje.** El backend de `bwrap` de la 139 monta `/repo`, `/salida`, `/oraculo`, `/cache`, `/rustsec` y `/trabajo` sobre `--ro-bind / /`. Con la raíz de solo lectura, `bwrap` no puede crear esas carpetas: `Can't create file /repo: Read-only file system`. En la Prueba 0d de Colab anduvo porque el orquestador las creó antes. `preparar-runtime.sh` (Tarea 140) no las crea.

**2. Los mensajes.** En modo `bwrap`, `error_de_plazo` dice "contenedor bwrap eliminado" y `error_de_infra` dice "podman falló". Nombran una herramienta que no corrió.

**3. Los repos.** La 141 omite del paquete los archivos con nombre de clave o token, también dentro de los repos. Un repo con `token.rs` deja de compilar en Colab. La calibración mediría otra cosa. El filtro por nombre tiene sentido en el `ROOT_HOME` y en las variables, no en el código de un repo.

**4. `n` y `semilla`.** `comando_de_trabajo` (`src/colab_calibra.rs`) corre `r00t oraculo calibrar --tarea <t> --dimension <d> --repo repos/<t> --json`, sin `--n` ni `--semilla`. Toma `n=20` por defecto. En la laptop, las dimensiones de oráculo se calibraron con `n=4` y semilla 42. Con 20, una calibración de Pruebas tarda unas cinco veces más.

**5. La vuelta.** La 141 baja los archivos de la vuelta y arma el paquete de vuelta en la laptop. El SHA-256 protege solo desde ese armado, no el viaje. Y si `stop` falla después de bajar la vuelta, la 141 no importa nada, aunque los datos llegaron bien.

## Tarea

La regla que manda: **una calibración en Colab corre igual que la del orquestador a mano, y su vuelta llega verificada desde el runtime.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Los puntos de montaje en el runtime

1. **`preparar-runtime.sh` crea** `/repo`, `/salida`, `/oraculo`, `/cache`, `/rustsec` y `/trabajo`, como carpetas vacías, antes de terminar. La lista vive en una sola variable, con un comentario que nombra los montajes de `argumentos_bwrap` (`src/oracle/contenedor.rs`).
2. **`verificar-versiones.sh` revisa** que las seis carpetas existan, con una línea `ok` o `mal` por cada una.
3. **Las rutas** se pueden cambiar con una variable, para los tests, como `ROOT_BIN_DIR`.

**Tests en rojo primero, en `tests/colab_preparar.rs`:**
- La verificación con las seis carpetas en una raíz temporal da `ok` para cada una.
- Sin `/repo`, la verificación da `mal` y lo nombra.
- Un test compara la lista del script con los destinos que monta `argumentos_bwrap`. Si `argumentos_bwrap` no se alcanza desde el test, compará con una lista fija, y anotalo.

### 2. Los mensajes de `bwrap`

1. **En modo `bwrap`,** el mensaje de plazo dice "proceso bwrap terminado" en vez de "contenedor bwrap eliminado".
2. **En modo `bwrap`,** el error de infraestructura dice "bwrap falló" en vez de "podman falló".
3. **En modo podman,** los mensajes no cambian.

**Tests en rojo primero:**
- Unitario: en modo `bwrap`, el error de plazo no contiene "contenedor".
- Unitario: en modo `bwrap`, el error de infraestructura no contiene "podman".
- Guarda: en modo podman, los dos mensajes son los de hoy.

### 3. Los repos enteros

1. **El filtro de archivos por nombre de clave o token** vale para lo que sale del `ROOT_HOME` y para las variables del entorno de la imagen.
2. **Los repos entran enteros,** salvo su carpeta `.git`.
3. **`root.toml` se sigue revisando** como hoy.

**Tests en rojo primero:**
- Unitario: un repo con `src/token.rs` lo lleva en el paquete. Hoy lo omite.
- Unitario: un repo con `.git/` no lo lleva.
- Guarda: un archivo `token.json` en el `ROOT_HOME` sigue fuera del paquete.

### 4. `n` y `semilla` por trabajo

1. **`r00t colab empaquetar` suma `--n` y `--semilla`,** con 4 y 42 por defecto. Valen para todos los trabajos del paquete.
2. **Un trabajo puede pedir otra `n`** con la forma `tarea:dimension:n`. Sin el tercer campo, usa `--n`.
3. **`comando_de_trabajo`** suma `--n <n>` y `--semilla <semilla>` al comando de cada trabajo.
4. **El manifiesto** guarda la `n` y la semilla de cada trabajo.
5. **Los dos valores por defecto** van en constantes, con su motivo: son los de la recalibración de oráculo de la laptop.

**Tests en rojo primero:**
- Unitario: sin `--n`, el comando de un trabajo lleva `--n 4 --semilla 42`. Hoy no lleva ninguno.
- Unitario: `s2-t1:pruebas:8` da `--n 8` para ese trabajo.
- Unitario: el manifiesto guarda la `n` y la semilla.

### 5. La vuelta armada en el runtime, e importar aunque falle el `stop`

1. **El guion de cierre del runtime** arma el paquete de vuelta ahí, con su propio manifiesto: el SHA-256 de cada archivo, calculado en el runtime.
2. **La laptop baja el paquete de vuelta** y revisa esos SHA-256 antes de importar. Un SHA-256 distinto frena la importación, como hoy.
3. **Si `stop` falla después de bajar la vuelta,** la importación corre igual. La sesión queda en `activo.json`, y el comando imprime un aviso que pide `oraculo pendientes --liberar`.
4. **Si `stop` falla antes de bajar la vuelta,** el comportamiento es el de hoy.

**Tests en rojo primero, con el falso de `ROOT_COLAB_BIN`:**
- Unitario: el guion de cierre escribe un manifiesto con los SHA-256 de lo que empaqueta.
- Unitario: una vuelta cuyo archivo no coincide con su SHA-256 del runtime no se importa.
- Unitario: un `stop` que falla después de bajar la vuelta deja las filas importadas, la sesión en `activo.json` y el aviso.

No corras la validez, `corre`, `calibrar` ni la pasada reales. No llames a Colab. Los corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab ni podman. Queda para el orquestador:
- **La prueba de la 141** del plan, vuelta (58), sección 5: `r00t colab calibrar` con un paquete de Seguridad de s2-t1, importando a una copia del `ROOT_HOME`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en el paquete, en un argumento ni en un log.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-145.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El comando de un trabajo**: el texto exacto, con `--n` y `--semilla`.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
