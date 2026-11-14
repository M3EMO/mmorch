# Brief de la Tarea 128 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-127`. Trae las Tareas 84 a 127.
- La 129 se construye a la vez. Toca `src/oracle/calibra/inyectar.rs` y el módulo de tests de `src/oracle/calibra/defectos.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-128`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, los casos sin los inestables | `src/oracle/mod.rs`, `src/oracle/calibra.rs` |
| 2, un defecto sin veredicto | `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 3, la rotación de defectos | `src/oracle/calibra.rs` |

- Los tres puntos tocan `calibra.rs`. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/defectos.rs`, `src/oracle/juez.rs`, `src/oracle/mutacion.rs`, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La medición P101 a de Diseño de s2-t1 dio sensibilidad 0,2 y cuatro limpios sin veredicto.** Costó 1,5921 USD. Los datos dicen tres cosas.

**1. La calibración mide la cobertura de otra forma que la evaluación.**
- La base marcó inestables `diseno.robustez` (peso 3) y `diseno.operabilidad` (peso 2).
- En una evaluación, `excluidos_de` (`src/oracle/mod.rs:1759`) los saca del peso total. La cobertura se calcula sobre 8.
- En una calibración, `excluidos_de` no saca nada. Su comentario dice: "Una medición de calibración no los quita". La cobertura de cada caso se calcula sobre 13.
- En tres limpios, el juez no dio evidencia para esos dos ítems. Midió 6 de 13, menos de dos tercios. Los tres quedaron sin veredicto.
- Con los inestables fuera, sería 6 de 8, que pasa los dos tercios.
- Después, `sin_inestables` saca esos ítems de los veredictos de los casos (`src/oracle/calibra.rs`, cerca de la línea 1585). Pero la cobertura ya cortó antes, dentro de la evaluación.

**2. Un defecto sin veredicto cuenta como no detectado.**
- Tres de los cinco defectos no tienen veredicto: dos por un corte de la API y uno por cobertura.
- La sensibilidad (`src/oracle/calibra.rs:1907`) divide los detectados por `inyecciones_distintas`, que incluye los defectos sin veredicto. Dio 1 de 5.
- La decisión P102 a del usuario cuenta "defectos distintos detectados" sobre "defectos distintos medidos". Un defecto sin veredicto no está medido. Con esa regla, la sensibilidad es 1 de 2.
- Los limpios ya siguen esa regla: la especificidad solo mira los limpios con veredicto, y `Version::calibrada` (`src/oracle/calibra.rs:395`) aplica `MAX_LIMPIOS_SIN_VEREDICTO_PCT`, el 20 %. Los defectos no tienen un tope así.

**3. Los cinco defectos cayeron en el piso.**
- Diseño tiene un solo defecto, `ErrorTragado`, sobre `diseno.errores`. Ese ítem vale 0 en la base.
- En Diseño, Arquitectura y Estructura, un ítem cuenta como bajado con 2 niveles (`BAJA_ITEM_CON_JUEZ`). Un ítem con nivel 0 o 1 no puede bajar.
- **Un defecto en el piso se puede detectar igual,** por la cita del juez. El caso 8 lo hizo. Por eso sigue contando como medido.
- La Tarea 129 suma defectos para ítems con nivel en la base. Cuando haya más de uno, la rotación debe preferir el que puede bajar. Hoy la rotación es fija: `defectos[(i / 2) % len]` (`src/oracle/calibra.rs:1261`).

## Tarea

La regla que manda: **la calibración mide la dimensión igual que la evaluación, y solo cuenta lo que midió.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Los casos sin los inestables

1. **Los casos con defecto y los limpios** se evalúan con los ítems inestables de la base excluidos, igual que una evaluación.
2. **La base no los excluye.** La base juzga con todos sus ítems, como hoy, para poder marcarlos.
3. **El paso del dato** sigue la forma de la calibración: un valor de hilo, como `con_plazo_fijo`. `excluidos_de` lo lee cuando `partes.dimension` está fijada. Actualizá su comentario.
4. **`sin_inestables` sigue,** después de la evaluación, como hoy.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: con `diseno.robustez` y `diseno.operabilidad` inestables en la base, un limpio que mide 6 de los 8 de peso estable tiene veredicto. Hoy queda sin veredicto con "cobertura 6 de 13".
- Unitario: la base se evalúa sin exclusiones.
- Unitario: fuera de una calibración, `excluidos_de` da lo de hoy.

### 2. Un defecto sin veredicto

1. **`inyecciones_distintas` cuenta solo los defectos distintos con veredicto.**
2. **`Fila` suma `defectos_sin_veredicto`,** con `#[serde(default)]`. Cuenta los defectos distintos sin veredicto.
3. **La sensibilidad** divide los detectados por los defectos distintos con veredicto.
4. **`Version` suma los defectos sin veredicto** de cada fila.
5. **`Version::calibrada`** pide que los defectos sin veredicto no pasen el 20 % de todos los defectos, igual que los limpios. Usa `MAX_LIMPIOS_SIN_VEREDICTO_PCT`. Si conviene, renombrala a un nombre que sirva a las dos clases, después de unir sus llamadores.
6. **Una fila vieja sin el campo** se lee con 0. Esas filas contaban los defectos sin veredicto como no detectados. Ese error las hace más estrictas, no más laxas.
7. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: cinco defectos, tres sin veredicto, uno detectado y uno no. La sensibilidad es 0,5, `inyecciones_distintas` es 2 y `defectos_sin_veredicto` es 3. Hoy la sensibilidad es 0,2.
- Unitario: una versión con 3 defectos sin veredicto de 10 no calibra, por el 20 %.
- Unitario: una versión con 2 de 10 sí puede calibrar.
- Unitario: una fila vieja sin `defectos_sin_veredicto` se lee con 0.

### 3. La rotación de defectos

1. **Un defecto puede bajar su ítem** si el nivel del ítem en la base alcanza la baja que pide su dimensión. En Diseño, Arquitectura y Estructura es `BAJA_ITEM_CON_JUEZ`. En las demás, un nivel.
2. **La rotación usa primero los defectos que pueden bajar su ítem,** en el orden de `de_dimension`.
3. **Si ninguno puede,** usa todos, como hoy.
4. **Un defecto sin nivel de base para su ítem** cuenta como que puede bajar. No hay datos para descartarlo.
5. `en_piso` y `defectos_en_piso` no cambian. Siguen informando.

**Tests en rojo primero:**
- Unitario: con dos defectos para una dimensión, y el ítem del primero en 0 en la base, todos los casos usan el segundo. Usá dos defectos de Seguridad, que hoy son dos, o un par de defectos falsos si la función lo permite.
- Unitario: con los dos ítems en el piso, la rotación es la de hoy.
- Guarda: con un solo defecto, la rotación es la de hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Los tests de calibración con `saltar_sin_podman`.
- **La repetición de P101 a para Diseño,** con juez y API, con la 128 y la 129. Espera la respuesta del usuario a P107.

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

Al terminar, escribí `docs/builds/informe-local-128.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Las filas viejas**: cuántas filas de los tests cambian su sensibilidad con la regla nueva, y por qué.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo renombrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
