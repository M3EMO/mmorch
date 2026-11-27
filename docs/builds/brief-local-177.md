# Brief de la Tarea 177 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (108).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-176c`** (ae50e34). Trae 5a4f295 y las Tareas 171 a 176c. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-177`.
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

**La prueba de aislamiento aisla-8 cortó antes del builder.** `r00t corre` salió con 3, a los 2 segundos:
"no se pudo lanzar el builder: fallo de infraestructura del builder (código 1): bwrap: Can't mkdir /opt/plugin: Read-only file system".

**La causa, en `src/corre/bwrap.rs`:**
- `RAIZ_SOLO_LECTURA` vale `["/usr", "/etc", "/opt"]` (línea 58). `args_bwrap_builder` monta cada una con `--ro-bind` si existe en el host (líneas 387 a 391).
- En Colab, `/opt` existe. En la laptop de los tests, no.
- Después, `args_bwrap_builder` monta el plugin de ECC en `PLUGIN_EN_CONTENEDOR`, que vale `/opt/plugin` (`src/corre/args.rs`, línea 15). bwrap tiene que crear `/opt/plugin`, y no puede: `/opt` ya es de solo lectura.
- Los tests de hoy arman una raíz sin `/opt`. Por eso no vieron el defecto.

**El builder no necesita nada del `/opt` de Colab.** `preparar-corre.sh` instala `node`, `claude` y Rust bajo `/usr`. La sonda 14 corrió `claude -p` de verdad sin montar `/opt`, y anduvo.

## La decisión

**`/opt` sale de `RAIZ_SOLO_LECTURA`.** El plugin sigue en `/opt/plugin`, que bwrap crea sobre su propia raíz, que es escribible.

**Por qué no mover el plugin a otra ruta:** `PLUGIN_EN_CONTENEDOR` también entra en los argumentos de `claude`, como `--plugin-dir /opt/plugin` (`src/corre/args.rs`, línea 46). Moverlo solo en bwrap obliga a que esos argumentos dependan del ejecutor. Así, `claude` recibiría argumentos distintos en la laptop y en Colab. Sacar `/opt` toca una sola constante, y deja iguales los argumentos de `claude` y el modo podman.

**Además, una guarda para que no vuelva a pasar:** ningún destino de un montaje del builder puede quedar dentro de una carpeta de la raíz que se montó en solo lectura.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, sacar `/opt` | `src/corre/bwrap.rs` |
| 2, la guarda de los destinos | `src/corre/bwrap.rs` |

- Los dos puntos van en serie: 1 y 2.
- **No toques `src/corre/args.rs`.** `PLUGIN_EN_CONTENEDOR` queda igual.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/colab_calibra.rs`, `src/corre/contenedor.rs`, `src/corre/ejecutor.rs`, `src/corre/args.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **El oráculo no cambia, y ninguna huella cambia.** Esta tarea solo toca el ejecutor de bwrap del builder.
- **El builder de podman de la laptop no cambia.**
- **Sin cambio de esquema,** y **sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.**
- Los tests de los argumentos usan la función `entrada` falsa que ya existe, para simular qué carpetas de la raíz existen.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **bwrap nunca tiene que crear una carpeta dentro de otra que ya montó en solo lectura.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Sacar `/opt`

1. **`RAIZ_SOLO_LECTURA` pasa a `["/usr", "/etc"]`.** Su comentario dice por qué `/opt` no está: el plugin se monta en `/opt/plugin`, y el builder no usa nada del `/opt` del host.
2. **El plugin sigue en `PLUGIN_EN_CONTENEDOR`,** con `--ro-bind`, como hoy.

**Tests en rojo primero:**
- Unitario: con una raíz falsa donde `/opt` existe, y con un plugin, los argumentos no traen `--ro-bind /opt /opt`, y sí traen `--ro-bind <plugin> /opt/plugin`. Hoy traen los dos.
- Guarda: con `/usr` y `/etc` existentes, los dos siguen montados con `--ro-bind`.
- Guarda: los argumentos de `claude` siguen trayendo `--plugin-dir /opt/plugin`.
- Guarda: los tests de `EjecutorContenedorBuilder` siguen verdes.

### 2. La guarda de los destinos

1. **Una función pura revisa los argumentos de bwrap ya armados.** Junta las carpetas montadas con `--ro-bind` desde la raíz del host: las de `RAIZ_SOLO_LECTURA` y las de `RAIZ_ENLACES` que son carpetas. Después, revisa el destino de cada montaje que sigue: `--bind`, `--ro-bind`, `--symlink`, `--proc`, `--dev` y `--tmpfs`.
2. **Si un destino queda dentro de una de esas carpetas,** la función da un error de infraestructura que nombra el destino y la carpeta. Por ejemplo: "el destino /opt/plugin queda dentro de /opt, montada en solo lectura".
3. **`lanzar` corre esa revisión antes de lanzar bwrap.** Así, el defecto sale con un mensaje claro, antes de bwrap, y no como un "Read-only file system".
4. **Una carpeta montada sobre sí misma no cuenta.** Por ejemplo, `--ro-bind /usr /usr` no choca con `/usr`.

**Tests en rojo primero:**
- Unitario: unos argumentos con `--ro-bind /opt /opt` y después `--ro-bind <x> /opt/plugin` dan el error, con los dos nombres. Hoy la función no existe.
- Unitario: los argumentos de una raíz falsa con `/usr`, `/etc`, `/opt`, `/bin` como carpeta y `/lib` como enlace, con plugin, credencial y `ROOT_PROC_BUILDER`, pasan la revisión.
- Unitario: un destino como `/usr/local/x` da el error.
- Unitario: con la revisión en `lanzar`, un conflicto frena el lanzamiento antes de llamar a bwrap. El bwrap falso no se ejecuta.
- Guarda: los montajes de `/trabajo`, del estado, de la credencial y del plugin no cambian.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/cadena-177`, que es la cadena 176c más esta tarea.
2. **`git diff laptop/cadena-176c laptop/tarea-177 --stat`** toca solo `src/corre/bwrap.rs` y el informe.
3. **La sonda de humo en Colab,** de la vuelta 108, antes de pedirle al usuario otro `paso0-aisla.sh`.
4. **La prueba de aislamiento en Colab,** con el release de `laptop/cadena-177` y `revisa.py` en el modo `b`.

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

Al terminar, escribí `docs/builds/informe-local-177.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los montajes del builder**: cada uno, con su origen, su destino y su modo, en el orden en que bwrap los hace.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
