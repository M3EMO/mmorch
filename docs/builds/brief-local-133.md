# Brief de la Tarea 133 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-132`. Trae las Tareas 84 a 132 y la 127b.
- La 134 se construye a la vez. Toca `src/oracle/calibra/inyectar.rs` y el módulo de tests de `src/oracle/calibra/defectos.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-133`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las medidas de cada caso | `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 2, el ancla de la referencia | `src/oracle/eficiencia.rs` |
| 3, la carpeta fija por lado | `src/oracle/eficiencia.rs`, `src/oracle/calibra.rs` |

- El punto 1 va primero, porque da los datos para ver los otros dos. El punto 2 y el 3 tocan `eficiencia.rs`, y van en serie después.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/defectos.rs`, `src/oracle/perfiles/`, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La calibración de Eficiencia de s2-t1 dio especificidad 0.** Los dos limpios dieron 0,950 y 0,948, contra una base de 1,0. Los dos quedaron detectados por `valor_menor`.

**La regla de detección no tiene margen.** `motivo_de_deteccion` (`src/oracle/calibra.rs`, cerca de la línea 3027) da `valor_menor` si `pv.valor < pb.valor`. Para Eficiencia, `estables::permite` siempre deja pasar.

**El ruido se mide, pero la calibración lo pierde.**
- La medición de la Tarea 115 marca regresión solo si la razón pasa 1 + ruido. El ruido es la dispersión de las tres corridas de la referencia, con un piso de 2 %.
- En una evaluación, los dos lados se alternan. Una carga pasajera pega en los dos.
- **En la calibración, la Tarea 131 reusa la referencia de la base.** Cada caso mide solo el repo, entre 10 y 40 minutos después de la base. Si la máquina cambió en ese tiempo, el repo sale más lento, y la dispersión de la referencia no lo ve.
- Un valor de 0,95 es una razón de 1,053. Pasa 1 + 2 %, y cuenta como regresión.

**No se sabe qué medida bajó.** Los `.casos.jsonl` guardan solo el valor de cada caso, no sus medidas.

**El tiempo de una tarea es de 38 a 50 minutos con n=4.** La Tarea 119 crea una carpeta de trabajo por medición, con un `target` por lado, y la borra al terminar. En la calibración, cada caso crea su carpeta de nuevo. El `target` del repo parte vacío, y la primera compilación sin medir compila todas las dependencias. Es el mismo problema que la Tarea 127 resolvió para la mutación, con una carpeta fija por calibración.

## Tarea

La regla que manda: **un caso de Eficiencia ve el cambio de la máquina en su propio ruido, guarda sus medidas, y no recompila las dependencias.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Las medidas de cada caso

1. **El renglón de cada caso** en `.casos.jsonl` suma `eficiencia`, con `#[serde(default)]`. Lleva el `eficiencia_auto` del veredicto del caso: cada medida, la referencia, el repo, la razón, el ruido, la regresión y el motivo.
2. **La base** también guarda sus medidas, donde la fila guarde datos de la base. Elegí el lugar y anotalo en el informe.
3. Un renglón viejo sin el campo se lee.
4. Regenerá `docs/interfaces.md` si cambia.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: un caso de Eficiencia guarda sus medidas en el renglón. Hoy no guarda ninguna.
- Unitario: un renglón viejo sin `eficiencia` se lee.

### 2. El ancla de la referencia

1. **Con la referencia fijada** (`con_referencia`, Tarea 131), cada caso corre también **una** corrida del lado `bootstrap/`, en medio de las del repo. El orden es: repo, inicial, repo, repo.
2. **Vale para** el tiempo de los tests, la memoria y la compilación. El tamaño sigue con la serie de la base, porque no cambia con el tiempo.
3. **El ruido de cada medida** es la dispersión de la serie de la base más el ancla, con el mismo piso de 2 %.
4. **La referencia que se compara** es el mínimo de la serie de la base más el ancla.
5. **Sin la referencia fijada,** la medición alterna como hoy. Una evaluación normal no cambia.
6. **Una función pura** recibe la serie de la base, el ancla y las corridas del repo, y da la razón, el ruido y la regresión.

**Tests en rojo primero:**
- Unitario: serie de la base 1,00, 1,01 y 1,00. Ancla 1,05. Repo 1,05, 1,06 y 1,05. Hoy: razón 1,05, ruido 2 % y regresión. Con el ancla: ruido 5 %, y sin regresión.
- Unitario: serie de la base 1,00, 1,01 y 1,00. Ancla 1,00. Repo 2,40, 2,41 y 2,42. Hay regresión, con razón 2,4.
- Unitario: con la referencia fijada, el ejecutor del lado inicial recibe una llamada por medida de tiempo, no tres.
- Guarda: sin la referencia fijada, el orden alterna como hoy.

### 3. La carpeta fija por lado

1. **Un valor de hilo nuevo en `eficiencia.rs`** guarda una carpeta de trabajo para la medición. Sigue la forma de `con_carpeta_de_mutacion` de la Tarea 127.
2. **Con ese valor,** cada lado usa su subcarpeta fija, en todas sus llamadas, en la base y en cada caso. La medición no la borra al terminar.
3. **Sin ese valor,** la medición crea y borra su carpeta como hoy.
4. **`calibra.rs`** crea la carpeta al empezar una calibración de Eficiencia, la fija en la base y en los casos, y la borra al terminar, también en un error, por `src/borrado.rs`.
5. **Las corridas medidas no cambian.** Cada una limpia el paquete con `cargo clean --release -p` antes de compilar, así que la medida es la misma.
6. **El lado del repo cambia de código entre casos.** El comando ya copia el repo de cada caso antes de compilar. Revisá que no quede código de otro caso. Si hace falta, borrá la copia del repo dentro del contenedor antes de copiar, como la Tarea 127.

**Tests en rojo primero, con un ejecutor falso que anota la carpeta:**
- Unitario: la base y dos casos reciben la misma carpeta para el lado del repo.
- Unitario: la carpeta no existe después de la calibración, también si un caso devuelve error.
- Unitario: sin el valor de hilo, la medición crea y borra su carpeta como hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **El reparto del tiempo** de una calibración de Eficiencia de s2-t1, con el envoltorio de podman de la vuelta (42), sección 6, y con `--dimension eficiencia`. Debe mostrar que la compilación en frío ocurre una vez por lado.
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 133 y la 134, en el `ROOT_HOME` real con respaldo. Los limpios deben dar 1,0. Las medidas de cada caso deben quedar en los renglones.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root en el host pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-133.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El orden de las llamadas de un caso**: cada llamada al ejecutor de cada lado, en orden, con su carpeta.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
