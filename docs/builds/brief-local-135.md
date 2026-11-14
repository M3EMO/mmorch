# Brief de la Tarea 135 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-134`. Trae las Tareas 84 a 134 y la 127b.
- La 136 se construye a la vez. Toca `src/oracle/calibra/inyectar.rs` y el módulo de tests de `src/oracle/calibra/defectos.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-135`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, las corridas guardadas | `src/tipos.rs` (solo `MedidaEficiencia`), `src/oracle/eficiencia.rs`, `docs/interfaces.md` |
| 2, `tiempo_tests` con 10 corridas y el ruido de los dos lados | `src/oracle/eficiencia.rs` |
| 3, una compilación del repo por caso | `src/oracle/eficiencia.rs` |
| 4, el aviso del ancla y el margen del test | `src/oracle/eficiencia.rs` |
| 5, el motivo de un lastre sin lugar | `src/oracle/calibra.rs` |

- Los puntos 1 a 4 tocan `eficiencia.rs`. Van en serie, en este orden. El punto 5 corre en paralelo con ellos.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/defectos.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La calibración de Eficiencia de t1 a t3 con la 133 guardó las medidas de cada caso.** Dicen cuatro cosas.

**1. El ruido de `tiempo_tests` se estima mal.**
- Las corridas de `tiempo_tests` duran entre 0,25 y 0,42 s. Un 2 % es de 5 a 8 ms.
- En t2, un limpio dio razón 1,024 con ruido 0,0204, y quedó como regresión.
- Las razones de los otros limpios y bases van de 0,991 a 1,005.
- Hoy el ruido sale solo de la referencia: tres corridas de la base, más el ancla. Las corridas del repo no entran. Con tres corridas cortas, la dispersión se estima mal.

**2. La compilación varía muy poco.** En los 12 casos, la razón de compilación quedó entre 0,995 y 1,003. La variación más grande fue 0,52 %. Cada compilación medida tarda entre 66 y 83 s. En una tarea con n=4 hay 22 compilaciones medidas, de 25 a 30 minutos.

**3. Un ancla que falla se ignora sin aviso** (pregunta 2 de la 133). Una falla escondida oculta una máquina rota.

**4. El test del ancla pasa por igualdad exacta de flotantes** (pregunta 5 de la 133). Con repo 1,05 y ruido 5 %, la razón queda justo en 1 + ruido.

**Un motivo que falta** (pregunta 2 de la 134). Cuando `LastreDeMemoria` no encuentra lugar, el caso sale con el motivo común `MOTIVO_SIN_CANDIDATO`. El motivo sale de `motivo_sin_inyeccion` en `src/oracle/calibra.rs`. Falta una rama propia para `LastreDeMemoria` con rangos del cambio.

## Tarea

La regla que manda: **el ruido de cada medida sale de datos suficientes de los dos lados, y la calibración no repite lo que ya sabe estable.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Las corridas guardadas

1. **`MedidaEficiencia`** suma `corridas_referencia` y `corridas_repo`, dos listas de números, con `#[serde(default)]`. Llevan cada corrida de cada lado, en el orden en que corrieron. El ancla va en `corridas_referencia`.
2. Una medida vieja sin los campos se lee.
3. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: una medición con un ejecutor falso guarda las corridas de los dos lados.
- Unitario: una medida vieja sin las listas se lee.

### 2. `tiempo_tests` con 10 corridas, y el ruido de los dos lados

1. **`tiempo_tests` corre 10 veces por lado.** Una constante lo fija, con su motivo: corridas de menos de medio segundo, donde 3 no alcanzan para estimar la dispersión.
2. **La memoria sale de las mismas corridas,** como hoy.
3. **La compilación sigue con su número de hoy,** salvo el punto 3.
4. **El ruido de una medida** es el mayor entre la dispersión de la referencia (con el ancla, si la hay) y la del repo. Sigue el piso de 2 %.
5. **La dispersión** es (máximo − mínimo) / mínimo, como hoy.
6. **La función pura de la comparación** recibe las corridas de los dos lados. Toda la aritmética sigue ahí.

**Tests en rojo primero:**
- Unitario: referencia de 10 corridas entre 0,400 y 0,404, y repo de 10 corridas entre 0,409 y 0,419. Hoy: ruido 2 % por el piso, razón 1,0225 y regresión. Con el cambio: el ruido sale del repo, cerca de 2,44 %, y sin regresión.
- Unitario: referencia y repo de 10 corridas estables, con el repo 50 % más lento. Hay regresión.
- Unitario: el ejecutor recibe 10 corridas de `tiempo_tests` por lado.

### 3. Una compilación del repo por caso

1. **En un caso de calibración, con la referencia fijada,** la compilación del repo corre una vez, no tres. El ancla sigue.
2. **La base de la calibración** sigue con 3 por lado. Fija el ruido.
3. **Una evaluación normal** sigue con 3 por lado.
4. **El ruido de la compilación del caso** sale de la serie de la base más el ancla, con el piso. La corrida única del repo no da dispersión.
5. **Un comentario** cita el dato: en 12 casos, la compilación varió como mucho 0,52 %, contra un piso de 2 %.

**Tests en rojo primero:**
- Unitario: con la referencia fijada, el ejecutor del repo recibe una compilación medida. Hoy recibe tres.
- Unitario: sin la referencia fijada, recibe tres por lado.

### 4. El aviso del ancla y el margen del test

1. **Si el ancla falla,** la referencia sigue con la serie de la base, como hoy, y el veredicto suma un aviso con la medida y el error. Usá la lista de avisos que `medir` ya recibe.
2. **El test del ancla** de la 133 usa un repo de 1,04, con margen, en vez de 1,05.

**Tests en rojo primero:**
- Unitario: un ancla que falla deja un aviso con la medida. Hoy no deja nada.

### 5. El motivo de un lastre sin lugar

1. **`motivo_sin_inyeccion`** suma una rama para `LastreDeMemoria` con rangos del cambio.
2. **El motivo** es "ninguna función vieja que corran los tests del estado inicial". Sirve para la regla de la 134 y para la de la 136.
3. Los demás motivos no cambian.

**Tests en rojo primero:**
- Unitario: un caso de `LastreDeMemoria` sin lugar, con rangos, da ese motivo. Hoy da `MOTIVO_SIN_CANDIDATO`.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 135 y la 136, en el `ROOT_HOME` real con respaldo. Los limpios no deben quedar detectados. Cada medida debe traer sus corridas.
- Que una tarea con n=4 tarde unos 10 minutos menos que en `efi-real-2/`.

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

Al terminar, escribí `docs/builds/informe-local-135.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las llamadas de un caso**: cada llamada al ejecutor de cada lado, en orden, con cuántas corridas tiene.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
