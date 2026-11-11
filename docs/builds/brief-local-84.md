# Brief de la Tarea 84 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-83` (86f260c). Las Tareas 82 y 83 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-84`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, credencial y señales | `src/corre/corrida.rs`, `src/corre/mod.rs`, `src/corre/ejecutor.rs` |
| 2, lecturas con tope | `src/oracle/contenedor.rs`, `src/corre/ejecutor.rs`, `src/oracle/colab.rs` |
| 3, `STEP` de un build grande | `src/oracle/entrega.rs`, `src/oracle/contenedor.rs` |
| 4, cierre de la Tarea 83 | `src/oracle/calibra.rs`, `src/oracle/mod.rs` (solo donde `evaluar` lee los inestables), `src/lib.rs`, `tests/calibra.rs`, `docs/interfaces.md` |
| 5, borrados | `src/oracle/juez.rs`, `src/observe/uso.rs` |

**Orden.**
- El punto 2 va primero.
- El punto 3 va después del 2, porque comparten `contenedor.rs`.
- El punto 1 va después del 2, porque comparten `ejecutor.rs`.
- Los puntos 4 y 5 corren en paralelo con todos.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`. `signal-hook` ya está, y `src/oracle/colab.rs` lo usa.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

La revisión de cierre del hito 1 encontró 5 hallazgos HIGH. Esta tarea cierra tres. Además, cierra cuatro preguntas del informe de la Tarea 83 y borra código sin uso. D3 y D4 van a correr `corre` y `evaluar` muchas veces sobre repos ajenos.

**HIGH 1, credencial.** `GuardiaCredencial` (`src/corre/corrida.rs:743`) borra la copia de la credencial en `Drop`. `r00t corre` no maneja señales. Un Ctrl-C, un SIGTERM o un SIGKILL dejan `claude-estado/.credentials.json` con el `accessToken`.

**HIGH 4, lecturas sin tope.** `leer_cola_en_hilo` ya lee con tope. Quedan dos lecturas sin tope:
- `leer_en_hilo` (`src/oracle/contenedor.rs:329`), que usa `src/corre/ejecutor.rs:337` para el builder.
- `leer_todo` (`src/oracle/colab.rs:1878`), que se usa en `colab.rs:463`.

Un proceso que imprime sin fin agota la memoria.

**HIGH 2, `STEP` de un build.** El commit 571d3e5 ya dejó el build con una cola de 1 MiB, y `clasificar_fallo` la lee. Queda un caso: un `RUN` que imprime más de 1 MiB después de su línea `STEP k/n:`. La cola pierde el `STEP`, y la sonda sale `Infra` en vez de "la imagen no construye".

**Preguntas 2, 3, 7 y 10 de la Tarea 83.**
- Pregunta 2: una dimensión inestable usa `no_aplica`, que quiere decir otra cosa.
- Pregunta 3: la base se juzga 3 veces aunque el juez no corra en la tarea.
- Pregunta 7: `evaluar` usa los inestables de la última fila sin mirar su huella.
- Pregunta 10: `Presupuesto.gasto_desde` quedó sin lectura.

**Borrados.** `cota_bytes_de_prompt`, `casos_con_reserva_abierta` y `casos_con_reserva_abierta_en` no tienen uso en producción.

## Tarea

La regla que manda: **ningún camino de salida deja la credencial en disco, y ninguna lectura crece sin límite.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Credencial y señales

1. **Señales.** `r00t corre` registra SIGINT y SIGTERM con `signal-hook`, como `colab.rs`. Al recibir una, borra la copia de la credencial, quita el contenedor del builder con plazo, y sale con código 130 para SIGINT y 143 para SIGTERM.
2. **Hilo, no manejador.** El borrado corre en un hilo con `signal_hook::iterator::Signals`, no dentro del manejador de la señal.
3. **Barrido al arrancar.** Antes de preparar una corrida nueva, `corre` busca copias `claude-estado/.credentials.json` en las corridas existentes.
4. **Corrida huérfana.** El pid va en el id de la corrida (`src/corre/mod.rs:113`). Una corrida es huérfana si ese pid no existe en `/proc`, o si su `/proc/<pid>/cmdline` no es de `r00t`.
5. **Borrado.** La copia de una corrida huérfana se borra por `src/borrado.rs`. Un enlace se borra como enlace, como hoy en `quitar_credencial`.
6. **`--reanuda`** también barre antes de reanudar.
7. **Sin cambio** en el camino normal ni en el de error: `GuardiaCredencial` sigue igual.

**Tests en rojo primero:**
- Integración: un `corre` con el builder falso recibe SIGTERM en medio de la corrida. La copia de la credencial no existe después, y el código de salida es 143.
- Unitario: una corrida con un pid que no existe en `/proc` y con copia de credencial queda barrida.
- Unitario: una corrida con el pid del proceso de test, que no es `r00t`, queda barrida.
- Guarda: una corrida viva de `r00t` no se barre. Simulá `/proc` con una raíz inyectable.
- Guarda: el camino normal y el de error siguen borrando la copia.

SIGKILL no se puede atrapar. Ese caso lo cubre el barrido.

### 2. Lecturas con tope

1. **`leer_en_hilo`** lee hasta un tope de 64 MiB. Después sigue drenando el flujo sin guardar nada, para que el proceso no se bloquee. Marca la lectura como recortada.
2. **El builder de `corre`** necesita su stdout completo para leer costos y sesión. Si su lectura sale recortada, la corrida falla con el error "salida del builder mayor que 64 MiB". Falla cerrado.
3. **`leer_todo` de `colab.rs`** usa el mismo lector con tope de 16 MiB. Una salida recortada se informa con un aviso, y conserva la cola.
4. Los topes van como constantes con su motivo en el comentario.

**Tests en rojo primero:**
- Unitario: un flujo de 70 MiB deja 64 MiB guardados, marca de recorte y el flujo drenado hasta el final.
- Unitario: un builder falso que imprime más del tope da el error de salida mayor que el tope.
- Unitario: la lectura de `colab.rs` con más de 16 MiB conserva la cola y avisa.
- Guarda: una salida chica se lee completa, igual que hoy.

### 3. `STEP` de un build grande

1. El lector del build guarda aparte la última línea `STEP k/n:` que vio, sin importar la cola.
2. `clasificar_fallo` usa esa línea si la cola ya no trae ningún `STEP`.
3. La cola de 1 MiB no cambia.

**Tests en rojo primero:**
- Unitario: un build falso con `sh` imprime `STEP 3/5: RUN x`, después 2 MiB de texto, y falla. `clasificar_fallo` da "la imagen no construye" en el paso 3, no `Infra`.
- Guarda: el test de 571d3e5 sigue verde.

### 4. Cierre de la Tarea 83

1. **Pregunta 2.** `Fila` suma `inestable: Option<String>`, con `#[serde(default)]`. Una dimensión inestable lo llena, y deja `no_aplica` en `None`. Una fila vieja con `no_aplica` que empieza con "inestable en esta tarea" se lee como inestable.
2. **Pregunta 3.** Si el juez no corre en esa tarea, la base se juzga una sola vez. El juez no corre si no hay modelo o si las rúbricas no le piden ítems.
3. **Pregunta 7.** `evaluar` usa la lista de inestables solo de una fila con huella vigente. Con otra huella, la lista no se usa.
4. **Pregunta 10.** Borrá `Presupuesto.gasto_desde` y su armado en `src/lib.rs` y en `tests/calibra.rs`.

**Tests en rojo primero:**
- Unitario: una dimensión inestable deja `inestable` lleno y `no_aplica` en `None`.
- Unitario: una fila vieja con `no_aplica = "inestable en esta tarea: ..."` se lee como inestable.
- Unitario: una tarea sin modelo de juez juzga la base una sola vez.
- Unitario: `evaluar` con una fila de huella vieja no excluye sus inestables.
- Guarda: con huella vigente, los inestables se excluyen como hoy.

### 5. Borrados

1. Borrá `cota_bytes_de_prompt` (`src/oracle/juez.rs:711`) y sus 4 tests.
2. Borrá `casos_con_reserva_abierta` y `casos_con_reserva_abierta_en` (`src/observe/uso.rs:108` y `:120`) y sus tests.
3. Antes de borrar, uní los llamadores de codegraph y de grep. Si alguno tiene un llamador de producción, no lo borres y explicalo.

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

Al terminar, escribí `docs/builds/informe-local-84.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Funciones borradas**: con sus llamadores revisados.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
