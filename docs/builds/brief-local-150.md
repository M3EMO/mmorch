# Brief de la Tarea 150 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (61).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-147`. Trae las Tareas 84 a 147 y la 127b.
- **Arrancá cuando el orquestador te pase el resultado de la prueba de la 141 en Colab.** Si esa prueba muestra un defecto nuevo en tus archivos, el orquestador lo suma a este brief.
- La 148 y la 149 pueden correr a la vez. Tocan `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs`, `src/oracle/calibra.rs` y `src/oracle/eficiencia.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-150`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las rutas absolutas y la subida en trozos | `src/oracle/colab.rs`, `src/colab_calibra.rs` |
| 2, el texto del SHA distinto | `src/colab_calibra.rs` |
| 3, `empaquetar_cli` fuera | `src/colab_calibra.rs`, `src/lib.rs` |
| 4, Eficiencia aceptada en Colab | `src/colab_calibra.rs` |
| 5, las variables de `verificar-versiones.sh` | `tools/colab/verificar-versiones.sh` |

- Los puntos 1 a 4 comparten `src/colab_calibra.rs`. Van en serie, en este orden. El punto 5 corre en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 148 y la 149, `src/oracle/contenedor.rs`, `src/tipos.rs`, `tools/colab/preparar-runtime.sh`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**1. La subida usa rutas remotas relativas.** `PAQUETE_REMOTO_IDA`, en `colab_calibra.rs`, vale `paquete.tar.gz`. El guion de preparación lo busca en `/content/paquete.tar.gz`. En la prueba de la 140, una ruta relativa no dejó el archivo en `/content`. El paquete entero de 182 MB, con ruta relativa, dio "400 Bad Request". Con trozos de 40 MB y rutas `/content/paq.NN`, la subida anduvo en 58 s, y el SHA-256 coincidió. `PAQUETE_REMOTO`, en `src/oracle/colab.rs`, tiene el mismo defecto con `root-paquete.tar.gz`. No sabemos si el 400 vino del tamaño o de la ruta. Por eso la subida va siempre en trozos, con rutas absolutas.

**2. Un SHA distinto dice algo falso.** Desde la 147, una vuelta con otro SHA-256 no se importa. Pero el texto termina con "su paquete de vuelta no llegó", que agrega `ciclo`. El paquete sí llegó.

**3. Dos entradas hacen lo mismo.** `lib.rs` llama a `empaquetar_cli` con los valores por defecto, y a `empaquetar_cli_con` con banderas. `empaquetar_cli_con` con 4 y 42 hace lo mismo que `empaquetar_cli`.

**4. Eficiencia queda rechazada en Colab.** `validar_trabajos` da "Eficiencia necesita el cgroup del contenedor". La 149 cambia la memoria y la carga para que no necesiten el cgroup. El usuario decidió calibrar Eficiencia en Colab (P111 b).

**5. La verificación de versiones falla fuera de la preparación.** `preparar-runtime.sh` instala Rust con `RUSTUP_HOME=/usr/local/rustup` y `CARGO_HOME=/usr/local/cargo`. `verificar-versiones.sh` agrega `/usr/local/cargo/bin` al PATH. Pero ese `rustc` es el proxy de rustup. Sin `RUSTUP_HOME`, el proxy busca en `~/.rustup` y no encuentra el toolchain. Una llamada aparte dio "mal rustc: 1.99.1 contra ausente" y "mal clippy". `script_cierre_en` llama así al guion, en cada vuelta.

## Tarea

La regla que manda: **lo que se sube llega a una ruta fija y se verifica, y un mensaje dice lo que pasó.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el borrado, que empieza con su guarda.

### 1. Las rutas absolutas y la subida en trozos

1. **Toda ruta remota de `upload` y `download` es absoluta,** bajo `/content`. Vale en `colab_calibra.rs` y en `colab.rs`.
2. **Una sola función sube un archivo en trozos.** Vive en `colab.rs`, y la usan los dos archivos. Parte el archivo en trozos de `TROZO_SUBIDA = 40 MiB`. Su comentario cita la prueba de la 140.
3. **Los trozos se llaman `/content/root-paq.NN`,** con `NN` de dos cifras o más, en orden. Un archivo chico va en un solo trozo, por el mismo camino.
4. **Los trozos se escriben en un temporal de Root,** nunca en `/tmp` del host. Se borran con `src/borrado.rs` al terminar, también con error.
5. **El guion de preparación une los trozos en orden** en la ruta que espera hoy. Después revisa el SHA-256 contra el valor que Root le pasa. Si difiere, falla con su marca y no abre nada. Después borra los trozos.
6. **Un trozo que no sube** frena la subida, con el número del trozo en el mensaje.
7. **Los binarios que sube `colab.rs`** también usan rutas absolutas. Si alguno pasa de 40 MiB, usa la misma función.

**Tests en rojo primero:**
- Unitario: con el `ColabCli` falso, cada `upload` y cada `download` lleva una ruta remota que empieza con `/`. Hoy `paquete.tar.gz` no empieza con `/`.
- Unitario: un archivo de 100 MiB sube en 3 trozos, en orden, con nombres `/content/root-paq.00` a `.02`.
- Unitario: un archivo de 1 KiB sube en un solo trozo.
- Unitario: el guion de preparación une los trozos y compara el SHA-256 con el valor que recibe.
- Unitario: un trozo que falla frena la subida y nombra el trozo.

### 2. El texto del SHA distinto

1. **Un SHA-256 distinto tiene su propio texto:** "su paquete de vuelta llegó con otro SHA-256 y no se abrió". Nombra los dos SHA, como hoy.
2. **El sufijo "su paquete de vuelta no llegó"** queda solo para una vuelta que de verdad no bajó.

**Test en rojo primero:**
- Unitario: con un `ROOT_SHA_VUELTA` distinto, el error no contiene "no llegó". Hoy lo contiene.

### 3. `empaquetar_cli` fuera

1. **`lib.rs` llama siempre a `empaquetar_cli_con`,** con los valores de las banderas o los de por defecto.
2. **Borrá `empaquetar_cli`.** Uní sus llamadores con codegraph y `grep -w` antes de borrar.

**Guarda:**
- Sin banderas, `colab empaquetar` llega a `empaquetar_cli_con` con 4 y 42, como fija la 146.

### 4. Eficiencia aceptada en Colab

1. **`validar_trabajos` deja de rechazar Eficiencia.**
2. **Los trabajos con ítems del juez** siguen rechazados, como hoy.

**Tests en rojo primero:**
- Unitario: un trabajo `s2-t1:eficiencia:4` pasa `validar_trabajos`. Hoy falla con "necesita el cgroup del contenedor".
- Guarda: un trabajo de una dimensión con juez sigue rechazado.

### 5. Las variables de `verificar-versiones.sh`

1. **El guion fija `RUSTUP_HOME` y `CARGO_HOME`** si no vienen, con los mismos valores que `preparar-runtime.sh`: `/usr/local/rustup` y `/usr/local/cargo`.
2. **Un comentario nombra la copia,** como hace hoy con `CARPETAS_RUNTIME`. Si cambia una, cambia la otra.
3. **El orden del PATH no cambia.** Un `rustc` del PATH sigue ganando.

**Test en rojo primero:**
- Unitario: el guion, corrido sin `RUSTUP_HOME` en el entorno, usa `/usr/local/rustup`. Probalo con un `rustc` falso en una carpeta temporal, que imprime su `RUSTUP_HOME`. Hoy imprime vacío.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Resultado de la prueba de la 141 (nota del orquestador)

La prueba corrió con el binario 9a6a10a y terminó antes de esta tarea.

- `colab calibrar` falló en la subida, con "no se pudo subir el paquete: ; su paquete de vuelta no llegó".
- **El mensaje sale vacío tras los dos puntos.** La CLI de Colab escribe sus fallas en stdout, como "[colab] Upload failed: 400 ...". Root arma el mensaje solo con `stderr`. Todo error de una llamada a la CLI debe incluir también su stdout, sin el token de la URL. La URL de la falla trae `colab-runtime-proxy-token=...`: ese valor no se escribe en ningún mensaje ni registro.
- **El tamaño sí importa.** El paquete entero de 182 MB a la ruta absoluta `/content/paquete.tar.gz` dio "500 Internal Server Error". Los trozos de 40 MiB a `/content/paq.NN` anduvieron. La subida en trozos es obligatoria.
- El runtime de la prueba quedó parado, y `activo.json` no quedó con sesiones.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 148, la 149 y la 150.
- La prueba de la 141 en Colab otra vez, con la subida en trozos. Ahí se ve que `verificacion.txt` del cierre da `ok`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-150.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lo que se borró**: cada función, con sus llamadores antes de borrar.
5. **Las rutas remotas**: cada `upload` y cada `download`, con su ruta antes y después.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
