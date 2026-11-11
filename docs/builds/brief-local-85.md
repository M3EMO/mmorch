# Brief de la Tarea 85 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-84` (1e68899). Las Tareas 82, 83 y 84 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-85`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, orden de la señal | `src/corre/corrida.rs`, `src/corre/ejecutor.rs` |
| 2, barrido | `src/corre/barrido.rs` |
| 3, lector de cola único | `src/oracle/contenedor.rs`, `src/oracle/colab.rs` |
| 4, `SalidaPodman.paso` | `src/oracle/contenedor.rs`, `src/oracle/entrega.rs`, y los literales de `SalidaPodman` en `src/oracle/descarga.rs`, `src/oracle/descarga/python.rs` y `src/oracle/mod.rs` |
| 5, prefijo y comentario | `src/oracle/calibra.rs`, `src/oracle/calibra/inestables.rs`, `src/oracle/juez.rs` |

**Orden.**
- El punto 4 va después del 3, porque comparten `contenedor.rs`.
- Los puntos 1, 2 y 5 corren en paralelo con todos.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`. La Tarea 84 mostró que codegraph solo no ve el llamador de `barrer`.

## Por qué

Son respuestas del planner al informe de la Tarea 84.

**Orden de la señal.** La credencial entra al contenedor como un archivo montado: `<copia>:/home/builder/.claude/.credentials.json:ro,Z` (`src/corre/contenedor.rs:268`). Un bind mount de un archivo apunta al inodo. Borrar la copia en el host quita el nombre, pero el builder puede seguir leyendo el token hasta que el contenedor muere. Hoy `al_recibir_senal` (`src/corre/corrida.rs:797`) borra primero y quita el contenedor después. Además, si la señal llega antes de que podman registre el contenedor, quitarlo por nombre no alcanza.

**Barrido.**
- Un `cmdline` ilegible cuenta hoy como vivo. Un `cmdline` ilegible es de otro usuario, así que no es un `r00t` nuestro.
- Un pid reusado por otro `r00t` vivo hoy no se barre.

**Lector de cola duplicado.** `leer_con_tope` en modo cola repite la lógica de `leer_cola_en_hilo_con`. La cola de colab puede empezar a mitad de un carácter.

**`STEP` antepuesto.** Hoy el lector del build antepone la última línea `STEP` al stdout. Mezclar texto reordenado en el stdout confunde al próximo lector.

**Prefijo duplicado.** "inestable en esta tarea" vive como constante en `calibra.rs` y como literal en `calibra/inestables.rs`. El comentario de `MAX_ENTRADA_JUEZ_BYTES` (`src/oracle/juez.rs:45`) habla de una cota que ya se borró.

## Tarea

La regla que manda: **el token deja de ser legible antes de que Root salga, y cada dato vive en un solo lugar.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Orden de la señal

1. **Primero el contenedor.** Al recibir la señal, Root termina el proceso `podman run` hijo y quita los contenedores del builder por nombre, con `PLAZO_AL_SENAL`.
2. **El hijo.** `ejecutor.rs` guarda el pid del hijo `podman run` en curso, para que el hilo de la señal lo pueda terminar. Sin hijo en curso, no hace nada.
3. **Después la copia.** Root borra la copia aunque el paso 1 falle o venza su plazo.
4. **Aviso.** Un fallo del paso 1 se escribe en stderr.
5. **Al final** sale con 130 o 143, como hoy.

**Tests en rojo primero:**
- Unitario: con un `podman` falso que anota sus llamadas, el orden registrado es: terminar el hijo, quitar contenedores, borrar la copia.
- Unitario: si quitar el contenedor falla, la copia se borra igual.
- Integración: una señal que llega antes de que el `podman` falso registre el contenedor termina el hijo. No queda ningún proceso del `podman` falso vivo.
- Guarda: el test de integración de SIGTERM de la Tarea 84 sigue verde.

### 2. Barrido

1. **`cmdline` ilegible:** la corrida es huérfana, y se barre.
2. **pid reusado:** el id de la corrida trae sus segundos de inicio (`src/corre/mod.rs:113`). Leé el inicio del proceso con ese pid desde `/proc/<pid>/stat`, campo 22, y `btime` de `/proc/stat`. Si el proceso empezó más de 2 segundos después de los segundos del id, no es la misma corrida, y se barre.
3. La raíz de `/proc` sigue inyectable, como en la Tarea 84.

**Tests en rojo primero:**
- Unitario: un `cmdline` ilegible da corrida huérfana.
- Unitario: un `r00t` vivo que empezó después de los segundos del id da corrida huérfana.
- Guarda: un `r00t` vivo que empezó antes de los segundos del id no se barre.

### 3. Lector de cola único

1. Un solo lector de cola. `leer_con_tope` en modo cola y `leer_cola_en_hilo_con` comparten la misma función.
2. La cola empieza en un borde de carácter UTF-8, como `recortar_cola` en `entrega.rs`.
3. El comportamiento externo no cambia: mismos topes, mismos avisos.

**Tests en rojo primero:**
- Unitario: una cola que cortaría un carácter de 3 bytes empieza en el carácter siguiente, sin carácter de reemplazo.
- Guarda: los tests de lectura con tope de la Tarea 84 siguen verdes.

### 4. `SalidaPodman.paso`

1. `SalidaPodman` suma `paso: Option<String>`. Lleva la última línea `STEP k/n:` que vio el lector del build, con su tope de 64 KiB.
2. El lector ya no antepone nada al stdout.
3. `clasificar_fallo` usa `paso` si la cola no trae ningún `STEP`.
4. Los demás literales de `SalidaPodman` llevan `paso: None`.

**Tests en rojo primero:**
- Unitario: un build con `STEP 3/5: RUN x` y después 2 MiB de texto deja `paso` lleno y un stdout sin la línea antepuesta.
- Guarda: el test de la Tarea 84 sigue dando "la imagen no construye" en el paso 3.
- Guarda: el test de 571d3e5 sigue verde.

### 5. Prefijo único y comentario

1. `calibra/inestables.rs` usa la constante de `calibra.rs` para "inestable en esta tarea". Borrá el literal y el test que fijaba que coincidían.
2. Corregí el comentario de `MAX_ENTRADA_JUEZ_BYTES`. Hoy es el límite de un prompt que se envía. Ya no alimenta ninguna cota de calibración.

**Tests:** este punto no cambia comportamiento. La guarda es la suite verde.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-85.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
