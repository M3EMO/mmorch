# Brief de la Tarea 119 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-117` con la 115. Trae las Tareas 84 a 115 y la 117.
- La 116 y la 118 pueden construirse a la vez. La 116 toca `src/corre/`, `src/tipos.rs` y `docs/interfaces.md`. La 118 toca `src/limpieza.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-119`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, un `/salida` fijo por lado | `src/oracle/perfiles/herramientas.rs`, `src/oracle/eficiencia.rs`, `src/oracle/mod.rs` |
| 2, la memoria sin `rustc` | `src/oracle/eficiencia.rs` |
| 3, un id desconocido dispara | `src/oracle/informe.rs` |
| 4, el empate del resumen | `src/oracle/informe.rs` |

- El punto 2 usa el punto 1. Van en serie. Los puntos 3 y 4 van en serie entre ellos, y en paralelo con el 1 y el 2.
- **Sin cambio de esquema.** No toques `src/tipos.rs` ni `docs/interfaces.md`. Si un punto lo pide, detenete y escribilo en el informe.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/tipos.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/contenedor.rs`, `src/limpieza.rs`, `src/corre/`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La Tarea 115 midió con un `target` frío en cada llamada.** `EjecutorContenedor::ejecutar` (`src/oracle/perfiles/herramientas.rs:89`) crea un `/salida` nuevo en cada llamada, con `carpeta_de_salida`. Por eso:
- **La memoria pico incluye a `rustc`.** El comando de tests compila con `--no-run` en el mismo contenedor, y `memory.peak` cuenta esa compilación. Si el repo compila más código, la razón sube sin que el producto use más memoria. Eso puede dar un "no listo" falso.
- **Cada corrida compila las dependencias otra vez.** Son 12 corridas por evaluación. En broot son minutos por corrida.

**Por qué no un campo nuevo en `EjecutorContenedor`.** El struct se arma con literal en `src/oracle/mod.rs:411`, en `src/oracle/calibra/defectos.rs:104` y en varios tests. Un campo nuevo los rompe a todos. Un método nuevo del trait, con un valor por defecto, no rompe nada.

**Dos huecos de la regla de la 115:**
- Una medida con un id que `informe.rs` no conoce, y razón mayor que 1,5, no dispara. Root falla cerrado: debe disparar.
- Con dos regresiones de igual razón, el resumen nombra la última del vector. El orden debe ser fijo.

## Tarea

La regla que manda: **cada lado compila una vez, la memoria mide solo los tests, y toda regresión grande dispara.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Un `/salida` fijo por lado

1. **El trait `EjecutorHerramienta`** suma `fn ejecutar_en(&self, cmd: &str, salida: &Path) -> Result<SalidaHerramienta, ErrorOraculo>`. Por defecto llama a `self.ejecutar(cmd)`. Los ejecutores falsos no cambian.
2. **`EjecutorContenedor`** lo implementa: monta `salida` en `/salida`, y lee sus archivos como hoy. No crea ni borra esa carpeta.
3. **`eficiencia::medir`** recibe una carpeta de trabajo. Crea una subcarpeta por lado, y usa `ejecutar_en` en todas las llamadas de ese lado.
4. **`mod.rs`** pasa una carpeta nueva dentro de `home/tmp`, con el mismo helper de temporales que usan las sondas.
5. **El borrado.** La carpeta se borra al terminar la medición, también en un error, por `src/borrado.rs`. Cada lado guarda un `target` de debug y uno de release. En broot pueden ser varios GB.
6. **Las dependencias se compilan una vez por lado.** La primera llamada de cada lado compila. Las corridas medidas reusan ese `target`.

**Tests en rojo primero, con un ejecutor falso que anota la carpeta de cada llamada:**
- Unitario: todas las llamadas de un lado reciben la misma carpeta. Hoy cada una usa una nueva.
- Unitario: los dos lados reciben carpetas distintas.
- Unitario: la carpeta de trabajo no existe después de `medir`, también si una llamada devuelve error.
- Unitario: el `ejecutar_en` por defecto llama a `ejecutar`.

### 2. La memoria sin `rustc`

1. **La compilación de tests va en su propia llamada,** con `cargo nextest run --no-run`. No lee la memoria.
2. **Cada corrida medida** es otra llamada, en un contenedor nuevo, con el mismo `/salida`. Corre solo los tests, y lee `/sys/fs/cgroup/memory.peak` al final.
3. **El reloj** sigue dentro del contenedor, con `date +%s%N`.
4. **Si la corrida medida compila algo,** la medida no vale. Detectalo con la salida de cargo, por ejemplo una línea `Compiling`. La memoria y el tiempo de esa corrida quedan sin valor, con el motivo "la corrida medida recompiló".
5. Las constantes y la aritmética de la 115 no cambian.

**Tests en rojo primero:**
- Unitario: el comando de la corrida medida no contiene `--no-run` ni `cargo build`.
- Unitario: la compilación de tests va en una llamada antes de las corridas medidas, una por lado.
- Unitario: una corrida medida con una línea `Compiling` en su salida deja la memoria sin valor, con su motivo.
- Guarda: los tests de `eficiencia.rs` de la 115 siguen verdes, salvo los que fijaban el comando viejo. Anotá esos en el informe.

### 3. Un id desconocido dispara

1. **Una medida con regresión y razón mayor que 1,5 dispara siempre** `eficiencia_regresion: <id>`.
2. **Con un id conocido,** la frase es la de hoy.
3. **Con un id desconocido,** la frase es genérica. Por ejemplo: "La medida <id> empeora 1,6 veces: 2,0 contra 1,25."

**Tests en rojo primero:**
- Unitario: una medida con id `otra`, regresión y razón 1,6 da "no listo", con `eficiencia_regresion: otra`. Hoy no dispara.
- Guarda: los tests de la regla de la 115 siguen verdes.

### 4. El empate del resumen

1. **Con dos regresiones de igual razón,** el resumen nombra la primera en este orden fijo: tiempo de los tests, memoria, compilación y tamaño.
2. Un id desconocido va después de los cuatro.
3. El orden vive en una sola constante.

**Tests en rojo primero:**
- Unitario: tiempo y tamaño con razón 1,3 los dos, y el tamaño primero en el vector. El resumen nombra el tiempo. Hoy nombra el tamaño.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Que `/sys/fs/cgroup/memory.peak` exista y se lea dentro del contenedor.
- Que el `target` fijo evite recompilar las dependencias entre corridas.
- Que `cargo clean --release -p <paquete>` limpie solo los paquetes del workspace.
- Que ninguna corrida medida dé "la corrida medida recompiló" en un repo real.
- Que la carpeta de trabajo desaparezca de `home/tmp` al terminar.
- **La prueba del orquestador:** una evaluación de t1 con Eficiencia, sin API. Anota las cuatro medidas, sus motivos, el tiempo total y el disco que usó la carpeta.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-119.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los comandos de cada medida**: el texto exacto de cada comando nuevo, y qué llamada usa cada carpeta.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
