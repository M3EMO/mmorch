# Brief de la Tarea 121 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-120`. Trae las Tareas 84 a 120.
- La 122 se construye a la vez. Toca los módulos de tests de `src/corre/corrida.rs` y `src/limpieza.rs`, y `tests/corre.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-121`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la evidencia de los juicios | `src/oracle/calibra.rs`, `docs/interfaces.md` |
| 2, un juicio sin veredicto | `src/oracle/calibra.rs`, `src/oracle/calibra/inestables.rs`, `docs/interfaces.md` |
| 3, la causa de cada inestable | `src/oracle/calibra/inestables.rs`, `src/oracle/calibra.rs`, `docs/interfaces.md` |

- Los tres puntos tocan `calibra.rs`. Van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/juez.rs`, `src/oracle/informe.rs`, `src/tipos.rs`, `src/lib.rs`, `src/corre/`, `src/limpieza.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La medición P101 a costó 0,4168 USD y no midió nada.**
- Diseño de s2-t1 dio n=0. Los seis ítems salieron inestables a la vez.
- Arquitectura de s2-t1 cortó con rc 3: "error de infraestructura del oráculo: la línea base no tiene veredicto de arquitectura: cobertura 6 de 10 de peso, menor que dos tercios".

**La causa de Diseño, en el código.**
1. La calibración juzga la base tres veces (`src/oracle/calibra.rs:1543`, `JUICIOS_EXTRA`).
2. Un juicio repetido puede perder ítems: el juez no da nivel, la cita no pasa el gate, o rompe el contrato. El reintento del juez no siempre los recupera (`src/oracle/juez.rs:754`).
3. Con menos de dos tercios de peso, `aplicar_cobertura` (`src/oracle/mod.rs:1735`) deja el puntaje sin veredicto. `sin_veredicto_puntaje` (`src/oracle/mod.rs:2085`) deja `items` vacío. Los `faltantes` quedan, con su motivo.
4. `inestables_de` (`src/oracle/calibra/inestables.rs:54`) marca inestable un ítem que falta en algún juicio. Con `items` vacío, faltan todos. Los seis salen inestables.
5. En `eventos.jsonl`, los juicios 2 y 3 necesitaron reintento. Encaja con esta causa.

**Un juicio sin veredicto no es una medida que varía.** Es una medida que falta. Root no debe leerlo como inestabilidad.

**La causa de Arquitectura.** La rúbrica pesa 10: `dependencias` (sonda, 3), `adecuacion` (juez, 3), `extensibilidad` (juez, 2) e `interfaces` (juez, 2). El primer juicio midió 6. Faltaron `extensibilidad` e `interfaces`. La calibración devuelve `ErrorOraculo::Infra` (`src/oracle/calibra.rs:1531`). No es infraestructura: es un resultado de la medición.

**Falta la evidencia.** La fila guarda `extractos_base`, pero no los niveles ni los faltantes de cada juicio. Sin ellos, no se sabe por qué faltó cada ítem. Las reglas de dos tercios y de inestables se deciden después, con esos datos.

## Tarea

La regla que manda: **la fila guarda lo que dijo cada juicio de la base, y un juicio sin veredicto no cuenta como inestabilidad.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La evidencia de los juicios

1. **`Fila` suma `juicios_base`,** con `#[serde(default)]`. Hay un elemento por juicio de la base, en orden.
2. **Cada elemento lleva:**
   - los niveles de la dimensión, de `Puntaje.items`;
   - los `faltantes` de la dimensión, con su ítem y su motivo;
   - el motivo `sin_veredicto` de la dimensión, si lo hay.
3. **Toda fila con juicios de la base los guarda.** Vale para una fila normal, una inestable y una del punto 2.
4. Una fila vieja sin el campo se lee.
5. Regenerá `docs/interfaces.md` si cambia.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: una calibración de Diseño con tres juicios guarda tres elementos en `juicios_base`, con sus niveles. Hoy no guarda nada.
- Unitario: un juicio con un faltante guarda el ítem y su motivo.
- Unitario: una fila vieja sin `juicios_base` se lee.

### 2. Un juicio sin veredicto

1. **`Fila` suma `base_sin_veredicto: Option<String>`,** con `#[serde(default)]`.
2. **Si cualquier juicio de la base queda sin veredicto en la dimensión,** la calibración no mide casos. La fila lleva `base_sin_veredicto` con "juicio <k> de la base sin veredicto: <motivo>". `k` cuenta desde 1.
3. **Ese caso no es inestable.** `inestable` e `inestables` quedan vacíos.
4. **Vale para el primer juicio.** Hoy el primero devuelve `ErrorOraculo::Infra` (`src/oracle/calibra.rs:1531`). Ahora escribe la fila y devuelve `Ok`.
5. **Una dimensión que no figura** en el veredicto de la base sigue igual, con su motivo de hoy, ahora en `base_sin_veredicto`.
6. **La fila no cuenta para calibrar.** `cuenta` (`src/oracle/calibra.rs`) excluye una fila con `base_sin_veredicto`.
7. **Un juicio repetido sin veredicto corta antes de juzgar otro.** Así no se gasta un tercer juicio que no sirve.
8. Regenerá `docs/interfaces.md` si cambia.

**Tests en rojo primero, con un evaluador falso:**
- Unitario: un segundo juicio sin veredicto, con `items` vacío y faltantes, da una fila con `base_sin_veredicto` "juicio 2 de la base sin veredicto: ...", y sin `inestables`. Hoy da los seis ítems inestables.
- Unitario: con el segundo juicio sin veredicto, el evaluador se llama dos veces, no tres.
- Unitario: un primer juicio sin veredicto devuelve `Ok` con la fila. Hoy devuelve `ErrorOraculo::Infra`.
- Unitario: `cuenta` no suma una fila con `base_sin_veredicto`.
- Guarda: tres juicios con niveles que varían 2 o más siguen dando la fila inestable de hoy.
- Guarda: los tests de `tests/calibra.rs` que esperaban el error de la base pasan al resultado nuevo. Anotá cada uno.

### 3. La causa de cada inestable

1. **`Fila` suma `causas_inestables`,** un mapa de ítem a texto, con `#[serde(default)]`.
2. **Las dos causas:**
   - "falta en el juicio <k>: <motivo>", con el motivo de sus `faltantes`. Sin faltante, el motivo es "sin nivel".
   - "varía de <mínimo> a <máximo>".
3. **Una función pura** de `inestables.rs` da las causas. Recibe los mismos juicios que `inestables_de`.
4. La regla de qué ítem es inestable no cambia.

**Tests en rojo primero:**
- Unitario: un ítem con niveles 1, 3 y 2 tiene la causa "varía de 1 a 3".
- Unitario: un ítem que falta en el juicio 3, con el faltante "sin evidencia que pase el gate", tiene la causa "falta en el juicio 3: sin evidencia que pase el gate".

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- Los tests de calibración con `saltar_sin_podman` que leen la fila.
- **La repetición de P101 a,** con juez y API, sobre la cadena con la 121. Cuesta cerca de 0,42 USD. La pide el planner aparte.

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

Al terminar, escribí `docs/builds/informe-local-121.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los campos nuevos de la fila**: su nombre, su tipo y un ejemplo en JSON.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
