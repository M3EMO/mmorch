# Brief de la Tarea 87 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-86` (70788ac). Las Tareas 84 a 86 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-87`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, hijo en estado compartido | `src/corre/ejecutor.rs`, `src/corre/corrida.rs` |
| 2, barrido | `src/corre/barrido.rs` |
| 3, piso y omitidos en la calibración | `src/oracle/calibra.rs`, `src/lib.rs` (solo la línea de texto de `calibrar`), `docs/interfaces.md` |

- Los tres puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función o de borrar un símbolo, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

**Hijo y señal.** La Tarea 86 puso el spawn y el registro bajo el cerrojo de `ESTADO_HIJO` (`src/corre/ejecutor.rs:321`). Quedan cuatro huecos:
- **Pid ya esperado.** El registro se borra después del `wait`. En esa ventana, el hilo de la señal puede mandar `kill` a un pid ya esperado, que el sistema pudo dar a otro proceso.
- **Error de `esperar`.** Si `esperar` da error, el hijo queda vivo con la credencial montada.
- **Segundo registro.** Un segundo hijo pisa al primero en silencio.
- **Copia después de la señal.** `lanzar` mira `terminando()` en la línea 186, y prepara la copia de la credencial en la línea 218. Si la señal llega entre las dos, el hilo de la señal borra la copia, y el hilo principal la crea otra vez antes de que el proceso salga.

**Barrido.**
- La tolerancia de 2 segundos contra `btime` es chica. Esta máquina es una laptop que se suspende, y NTP corrige el reloj. Una corrida viva puede verse como pid reusado y perder su copia.
- Un `gate(...)?` o un `?` en una entrada aborta todo el barrido (`src/corre/barrido.rs:82` y `:93`). Las demás corridas quedan sin barrer.

**Calibración.** En t1 de S2, el defecto de Diseño apunta a `diseno.errores`, que en la base ya está en nivel 0. Desde el piso, la vía de la baja del ítem no puede detectar. Las otras vías, sondas y cita del juez, sí. Hoy nada marca ese caso. Además, el juez puede dejar sin nivel un ítem estable en un caso, y eso no se cuenta.

## Tarea

La regla que manda: **un hijo esperado nunca se mata, y ninguna credencial se crea después de la señal.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Hijo en estado compartido

1. **El `Child` vive en `ESTADO_HIJO`.** El estado guarda el `Child` del `podman run` en curso, no solo su pid.
2. **Espera con `try_wait`.** El hilo principal espera con `try_wait` bajo el cerrojo, en un bucle con pausa corta, hasta el plazo de hoy. Al ver que terminó, saca el `Child` del estado en el mismo cerrojo.
3. **La señal termina por el `Child`.** El hilo de la señal toma el cerrojo y llama a `kill` sobre el `Child` guardado. Un `Child` ya esperado no está en el estado, así que nunca se mata un pid ajeno.
4. **Error de `esperar`.** Si la espera da error o vence el plazo, el hilo principal termina el hijo antes de devolver el error.
5. **Segundo registro.** Registrar un hijo con otro ya registrado da error, y no lanza nada.
6. **Copia bajo el cerrojo.** `lanzar` toma el cerrojo, mira `terminando`, prepara la copia de la credencial, hace el spawn y registra el hijo, todo sin soltarlo. Con `terminando`, no crea la copia.
7. **La señal conserva el cerrojo.** El hilo de la señal toma el cerrojo, marca `terminando`, termina el hijo, quita los contenedores, borra la copia y sale, sin soltar el cerrojo. Los comandos de limpieza no toman ese cerrojo, así que no hay bloqueo.
8. **Confirmar el MEDIUM 1.** Un test muestra que la limpieza de la señal no pisa el registro del hijo.

**Tests en rojo primero:**
- Unitario: con una costura entre el `wait` y el borrado del registro, una señal no manda `kill` a ningún pid.
- Unitario: un error de la espera deja al hijo terminado.
- Unitario: un segundo registro da error.
- Unitario: con una costura entre el chequeo de `terminando` y la copia, una señal en esa ventana no deja ninguna copia en disco.
- Unitario: la limpieza de la señal no cambia el registro del hijo.
- Guarda: los tests de señal de las Tareas 84 a 86 siguen verdes.

### 2. Barrido

1. **Tolerancia.** La diferencia permitida entre el inicio del proceso y los segundos del id sube de 2 a 300 segundos. El comentario explica el motivo: suspensión y NTP.
2. **Un error por corrida no aborta.** Un error al leer o al registrar el `gate` de una corrida se informa por stderr, y el barrido sigue con las demás.
3. El valor de vuelta lista las corridas barridas y las que dieron error.

**Tests en rojo primero:**
- Unitario: un `r00t` vivo que empezó 60 segundos después de los segundos del id no se barre.
- Unitario: un `r00t` vivo que empezó 400 segundos después sí se barre.
- Unitario: con tres corridas huérfanas y la del medio con un error, la primera y la tercera quedan barridas.

### 3. Piso y omitidos en la calibración

1. **`piso`.** Cada caso con defecto guarda `piso: bool`. Vale `true` si el nivel base del ítem apuntado es menor que la baja que su dimensión pide para detectar (`BAJA_ITEM_CON_JUEZ` en las dimensiones con juez, 1 en las demás).
2. **`omitidos`.** Cada caso guarda `omitidos: Vec<String>`: los ítems estables en la base que el juez dejó sin nivel en ese caso.
3. **Fila.** `Fila` suma `defectos_en_piso: u32` y `omisiones: u32`.
4. **Texto.** La línea de `calibrar` muestra los dos conteos.
5. **Sin cambio de fórmula.** La sensibilidad, la especificidad y la exactitud no cambian.
6. Todos los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un defecto sobre un ítem de Diseño con nivel base 0 guarda `piso: true`.
- Unitario: con nivel base 3, guarda `piso: false`.
- Unitario: un caso donde el juez no da nivel a un ítem estable lo lista en `omitidos`.
- Unitario: la fila cuenta `defectos_en_piso` y `omisiones`, y la línea de texto los muestra.
- Guarda: la sensibilidad de un defecto en el piso detectado por cita sigue contando como detección.
- Guarda: un caso viejo sin los campos se lee con `false` y con la lista vacía.

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

Al terminar, escribí `docs/builds/informe-local-87.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
