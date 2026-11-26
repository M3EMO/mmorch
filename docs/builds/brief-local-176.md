# Brief de la Tarea 176 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (103), (104) y (105).

**Este brief vale con P133 b.** El usuario respondió, literal: "b) Pids propios, /proc ro (Recommended)".

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-175`.** Trae 5a4f295 y las Tareas 171 a 175. La arma el orquestador después de la suite de la 175. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-176`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- **El builder arregla los defectos de Root que encuentra en esta tarea, por P121 a y P122 a.** Si un arreglo pide un archivo fuera de la tabla, se detiene y lo anota.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Por qué

**En Colab, nadie puede montar un procfs nuevo.** Las sondas 6 a 12 lo midieron:
- `bwrap --unshare-pid --proc /proc` falla con `Can't mount proc on /newroot/proc: Permission denied`;
- root tampoco puede montar un procfs, con ninguna opción;
- `--ro-bind /proc /proc` sí anda, con `--unshare-pid`.

**Hoy `src/corre/bwrap.rs` pasa siempre `--unshare-pid` y `--proc /proc`** (líneas 331 y 352). Por eso el builder no arranca en Colab.

**El usuario eligió la opción b (P133 b):** el builder tiene su propio espacio de pids, y ve el `/proc` del runtime en solo lectura. Es lo que ya hace el oráculo en `src/oracle/contenedor.rs`. Con esa opción:
- el builder ve la lista de procesos del runtime y sus líneas de comandos;
- el kernel le niega el `environ`, los `fd/`, la raíz y la memoria de los procesos de root, y no puede mandarles señales;
- sus procesos mueren con su espacio de pids cuando termina `claude`;
- `node`, `rustc`, `cargo` y `claude` andan igual que en la laptop.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el `/proc` del builder en bwrap | `src/corre/bwrap.rs` |
| 2, la variable en el runtime | `tools/colab/corre-runtime.py`, y su test en `src/colab_corre.rs` |

- Los dos puntos no comparten archivos. Van en serie: 1 y 2.
- **`tools/colab/preparar-corre.sh` no cambia.** No hay ningún montaje nuevo en el runtime.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/corre/contenedor.rs`, `src/corre/ejecutor.rs`, el resto de `src/`, `tools/colab/preparar-corre.sh`, `tools/colab/red-builder.sh`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **El builder de podman de la laptop no cambia.** Esta tarea solo toca el ejecutor de bwrap.
- **Ninguna huella cambia.** **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **Ningún test monta nada, ni corre `mount` real.** Los tests usan textos de `mountinfo` falsos.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **en bwrap, el builder tiene siempre su propio espacio de pids. Donde no se puede montar un procfs, ve el del runtime en solo lectura, y nunca uno que no sea un procfs.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El `/proc` del builder en bwrap

1. **Una variable nueva: `ROOT_PROC_BUILDER`.** Su valor es la ruta de un procfs ya montado. En Colab vale `/proc`.
2. **Con la variable,** los argumentos de bwrap cambian `--proc /proc` por `--ro-bind <ruta> /proc`.
3. **Sin la variable,** los argumentos quedan como hoy, con `--proc /proc`.
4. **`--unshare-pid` queda siempre,** con o sin la variable.
5. **El chequeo previo suma, con la variable:** la ruta es el punto de montaje de un procfs, según `/proc/self/mountinfo`. Es decir, una línea de `mountinfo` tiene esa ruta como punto de montaje, y `proc` como tipo de sistema de archivos.
   - Si la ruta no es un punto de montaje, o su tipo no es `proc`, es un error de infraestructura que nombra la ruta y lo que encontró.
   - **No se exige `hidepid`.** P133 b acepta que el builder vea la lista de procesos del runtime.
6. **La lectura de `mountinfo` es una función pura:** recibe el texto y la ruta, y devuelve el tipo del montaje o nada. Así se prueba sin montar nada.
7. **El veredicto de la corrida suma un aviso, con la variable:** "builder con /proc del runtime en solo lectura".

**Tests en rojo primero:**
- Unitario: con `ROOT_PROC_BUILDER=/proc`, los argumentos traen `--ro-bind /proc /proc`, y no traen `--proc`. Hoy la variable no existe.
- Unitario: sin la variable, los argumentos traen `--proc /proc`, como hoy.
- Unitario: con o sin la variable, los argumentos traen `--unshare-pid`.
- Unitario: un `mountinfo` falso con un procfs en `/proc` pasa el chequeo.
- Unitario: un `mountinfo` falso con un tmpfs en `/proc` frena el lanzamiento, con un error que nombra el tipo.
- Unitario: una ruta que no aparece como punto de montaje frena el lanzamiento.
- Unitario: con la variable, el veredicto trae el aviso.
- Guarda: los montajes de bwrap, salvo el de `/proc`, son los mismos que en la base.
- Guarda: los tests de `EjecutorContenedorBuilder` siguen verdes.

### 2. La variable en el runtime

1. **`corre-runtime.py` fija `ROOT_PROC_BUILDER=/proc`** en el entorno del `r00t` hijo, junto con las otras variables de bwrap.

**Tests en rojo primero:**
- Unitario de Python: el entorno del `r00t` hijo trae `ROOT_PROC_BUILDER=/proc`. Hoy no la trae.
- Guarda: el resto del entorno del hijo es el de la 174.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/cadena-176`, que es la cadena 175 más esta tarea.
2. **`git diff laptop/cadena-175 laptop/tarea-176 --stat`** toca solo `src/corre/bwrap.rs`, `tools/colab/corre-runtime.py`, `src/colab_corre.rs` y el informe.
3. **La prueba de aislamiento en Colab,** con el release de `laptop/cadena-176` y `revisa.py` en el modo `b`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-176.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El `/proc` del builder**: lo que ve en la laptop con podman, y en Colab con bwrap y la variable.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
