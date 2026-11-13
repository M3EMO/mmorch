# Brief de la Tarea 115 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-114`, en eee0aa0. Trae las Tareas 84 a 111, la 113 y la 114.
- La 117 está en `laptop/tarea-117`, y no está en tu base. No toques `src/oracle/contenedor.rs`, `src/oracle/descarga.rs`, `src/oracle/calibra/defectos.rs` ni `src/oracle/calibra/target_podman.rs`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-115`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, los comentarios viejos | `src/tipos.rs` (solo comentarios), `docs/interfaces.md` |
| 2, la medición | `src/oracle/eficiencia.rs` (nuevo), `src/oracle/mod.rs` (solo `mod eficiencia;`) |
| 3, el resultado en el veredicto | `src/tipos.rs`, `src/oracle/mod.rs`, `src/oracle/perfiles/herramientas.rs`, `docs/interfaces.md` |
| 4, la regla y el informe | `src/oracle/informe.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/contenedor.rs`, `src/lib.rs`, `src/corre/`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario sobre P95, literal:** "p95 por que no detecta el test rojo? la idea no es solo de funcionamiento si no de eficiencia. Si hay algo que haga mas lento de lo que deberia, mas pesado de lo que deberia u otra cosa entonces deberia dar mal o una escala continua."

**La decisión del usuario sobre P105, literal:** "a e intentar subir la eficiencia". La opción "a" decía: una regresión de más de 1,5 veces, que además pasa la variación propia de la referencia, da "no listo". Una regresión menor solo baja la escala.

**Lo que pasa hoy.** Eficiencia solo se mide si la tarea trae `[eficiencia]` (`eficiencia`, `src/oracle/perfiles/herramientas.rs:731`). Sin esa sección, da `no_aplica` con `MOTIVO_SIN_EFICIENCIA` (línea 728). Ninguna tarea de S2 la trae. Un cambio que hace los tests tres veces más lentos da "listo".

**La referencia es el estado inicial.** `bootstrap/` vive en `oraculos/<tarea>/bootstrap/`. La línea base de Seguridad ya corre sondas sobre ese estado, con su propio ejecutor (`src/oracle/mod.rs:798`, `previos::linea_base`). La medición nueva sigue el mismo camino.

**Lo que dice la investigación sobre el ruido.**
- rustc-perf compara el mínimo de 3 corridas. El mínimo es la medida de tiempo menos ruidosa.
- Perfherder usa un piso de 2 % de cambio.
- `perf` no anda en podman sin root. La memoria sale de `memory.peak` del cgroup v2.

**Los comentarios viejos.** La Tarea 113 renombró dos reglas. `src/tipos.rs:459` todavía nombra `test_oculto_fuera_de_requisitos`, y `src/tipos.rs:467` nombra `test_contrato_fuera_de_requisitos`. `docs/interfaces.md` repite esos nombres en 4 líneas.

## Tarea

La regla que manda: **un cambio que hace el producto más lento, más pesado o más grande que el estado inicial baja Eficiencia. Si lo empeora más de 1,5 veces, por encima del ruido, da "no listo".** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 1.

### 1. Los comentarios viejos

1. En `src/tipos.rs:459` y `:467`, los comentarios nombran `test_oculto_falla` y `test_contrato_falla`. Dicen que todo test estable que falla da `NoListo`, lo pruebe o no un requisito.
2. Regenerá `docs/interfaces.md`.
3. Este punto no cambia código. No necesita test en rojo.

### 2. La medición

Un módulo nuevo, `src/oracle/eficiencia.rs`, mide los dos lados con un `EjecutorHerramienta` por lado: uno sobre `bootstrap/` y otro sobre el repo evaluado. Solo en Rust.

**Las cuatro medidas:**

| Medida | Cómo | Corridas por lado | Valor que se compara |
|---|---|---|---|
| Tiempo de los tests | nextest, con un filtro que elige los tests que pasan en los dos lados | 3, alternadas | El mínimo |
| Memoria pico | `cat /sys/fs/cgroup/memory.peak` al final de cada corrida de tests | 3 | El mínimo |
| Tiempo de compilación | `cargo clean --release -p <paquete>` y después `cargo build --release --offline` | 3, alternadas | El mínimo |
| Tamaño del binario | La suma de los binarios de release de los paquetes del workspace | 1 | El tamaño |

1. **Los tests comunes.** Una primera corrida de nextest en cada lado da los tests que pasan. La medida usa solo los que pasan en los dos lados. Sin tests comunes, el tiempo y la memoria quedan sin valor, con el motivo "ningún test pasa en los dos lados".
2. **Alternadas.** El orden es: inicial, repo, inicial, repo, inicial, repo. Una carga pasajera pega en los dos lados.
3. **La memoria va en un contenedor que solo corre tests.** La compilación va antes, en otra llamada. Si no, el pico es el de `rustc`.
4. **Un `target` por lado.** Los dos lados tienen el mismo nombre de paquete. Un `target` compartido recompila en cada cambio de lado.
5. **Los paquetes y los binarios** salen de `cargo metadata --no-deps --format-version 1`. Un workspace sin binarios deja el tamaño sin valor, con el motivo "sin binarios".
6. **El tiempo** lo toma el comando dentro del contenedor, con `date +%s%N` antes y después. El arranque del contenedor no cuenta.
7. **El ruido** de una medida es la dispersión de la referencia: (máximo − mínimo) / mínimo. Tiene un piso de 2 %. El tamaño usa el piso.
8. **La razón** es la medida del repo dividida por la de la referencia.
9. **Hay regresión** si la razón pasa 1 + ruido.
10. **Una medida que no se puede tomar** queda sin valor, con su motivo. Por ejemplo, sin `memory.peak`, o una compilación que falla. Las otras medidas siguen.
11. **Una función pura** recibe las corridas de los dos lados y da la razón, el ruido y si hay regresión. Toda la aritmética vive ahí.
12. **Las constantes** van con su motivo: 3 corridas y el mínimo (rustc-perf), el piso de 2 % (Perfherder), y 1,5 veces (la decisión P105 a del usuario).

**Tests en rojo primero, con un ejecutor falso:**
- Unitario: referencia 1,0, 1,1 y 1,0 s, y repo 1,05, 1,06 y 1,08 s. La razón es 1,05, el ruido es 10 %, y no hay regresión.
- Unitario: referencia 1,0, 1,01 y 1,0 s, y repo 1,6, 1,7 y 1,65 s. La razón es 1,6, el ruido es 2 % por el piso, y hay regresión.
- Unitario: el orden de las llamadas al ejecutor alterna los dos lados.
- Unitario: un test que pasa solo en el repo no entra al filtro.
- Unitario: sin `memory.peak`, la memoria queda sin valor con su motivo, y el tiempo tiene valor.
- Unitario: un workspace sin binarios deja el tamaño sin valor.

### 3. El resultado en el veredicto

1. **`Veredicto` suma `eficiencia_auto`,** con `#[serde(default)]`. Lleva, por medida: la referencia, el repo, la razón, el ruido, si hay regresión, y el motivo si quedó sin valor.
2. **El valor de cada medida** es 1 sin regresión. Con regresión, es referencia / medida.
3. **Eficiencia** es el menor valor entre las medidas con valor. Si la tarea trae `[eficiencia]`, el valor del banco también entra al mínimo, y sus hallazgos siguen.
4. **Sin `bootstrap/`:** Eficiencia queda en `no_aplica`, con el motivo "sin estado inicial". Si la tarea trae `[eficiencia]`, el banco decide solo, como hoy.
5. **Fuera de Rust:** queda como hoy. Sin `[eficiencia]`, da `no_aplica`.
6. **Sin ninguna medida con valor y sin banco:** Eficiencia queda en `sin_veredicto`, con los motivos de las medidas.
7. **Cuándo corre.** Corre si `partes.mide(Dimension::Eficiencia)`. Una calibración de otra dimensión no la corre.
8. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: una tarea de Rust sin `[eficiencia]`, con `bootstrap/` y sin regresiones, da Eficiencia con valor 1. Hoy da `no_aplica`.
- Unitario: con una regresión de 1,3 veces en el tiempo, Eficiencia vale cerca de 0,77.
- Unitario: una tarea sin `bootstrap/` da `no_aplica` con el motivo "sin estado inicial".
- Unitario: con `[eficiencia]` y un banco que da 0,6, Eficiencia es el menor entre 0,6 y la medición automática.
- Unitario: un veredicto viejo, sin `eficiencia_auto`, se lee.
- Guarda: una calibración de Diseño no llama a la medición.

### 4. La regla y el informe

1. **Regla nueva en `no_listo`:** `eficiencia_regresion: <medida>`. Dispara por cada medida con regresión y razón mayor que 1,5.
2. **Su frase** nombra la medida y los dos números. Por ejemplo: "Los tests tardan 1,6 veces más: 1,62 s contra 1,01 s."
3. **La regla no espera la calibración,** igual que un test rojo.
4. **El resumen** suma una línea. Nombra la medida con la mayor razón entre las que tienen regresión, con los dos números. Sin regresiones, dice "Sin regresiones de eficiencia." Sin medición, no suma nada.
5. El orden de `decidir` no cambia: bloqueos, "no listo", "sin decisión" y "listo".

**Tests en rojo primero:**
- Unitario: una regresión de 1,6 veces por encima del ruido da "no listo", con la regla `eficiencia_regresion: tiempo_tests`.
- Unitario: una regresión de 1,6 veces dentro del ruido no dispara.
- Unitario: una regresión de 1,3 veces no dispara, y el resumen la nombra.
- Unitario: sin regresiones, el resumen dice "Sin regresiones de eficiencia.".
- Guarda: un secreto y una regresión de 2 veces dan "bloqueado".

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-115.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los comandos de cada medida**: el texto exacto de cada comando, y cómo se lee su salida.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
