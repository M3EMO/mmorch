# Brief de la Tarea 82 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (99c0f01). Las Tareas 80 y 81 ya están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-82`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, huella solo de sondas | `src/oracle/calibra.rs` |
| 2, ventana de cambio | `src/oracle/juez.rs` (solo `ventanas`, `extractos_de_cambios` y sus tests) |
| 3, motivo visible | `src/tipos.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs` y `docs/interfaces.md` |
| 4, cota del peor caso | `src/oracle/calibra.rs` y `src/oracle/juez.rs` (solo una función pública nueva junto a `bytes_de_prompt`) |

**Paralelo.**
- Los puntos 1 y 4 comparten `calibra.rs`. Van en serie: primero el 1.
- Los puntos 2 y 3 corren en paralelo con ellos.
- El punto 4 toca `juez.rs` solo con una función nueva, lejos de `ventanas`. Si choca con el punto 2, mezclá el 2 primero.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- Antes de cambiar la firma de una función, uní los llamadores de `codegraph callers <nombre>` y de `grep -w <nombre>`. En esta máquina el comando es `codegraph`. Si el worktree no tiene índice, corré `codegraph init .` y `codegraph sync`.

## Por qué

Son tres respuestas del planner al informe de la Tarea 81, más la cota de gasto.

1. **Huella.** `huella_de` (`src/oracle/calibra.rs:195`) junta el prompt del juez con las rúbricas en toda dimensión. Una dimensión solo de sondas, como Seguridad, no depende del juez. Hoy, cambiar el prompt descalibra Seguridad sin motivo.
2. **Ventana.** Una ventana de cambio de más de `extracto_max` se corta al final. La parte cambiada puede quedar fuera, y es justo lo que el juez debe leer.
3. **Motivo.** `Calibraciones::motivo_sin_calibrar` existe y tiene test, pero el veredicto no lo muestra. Un puntaje sin calibrar por huella vieja no dice por qué.
4. **Cota.** `peor_caso_de_caso` (`src/oracle/calibra.rs:1096`) usa 256 KiB fijos de entrada, y cuenta un solo pedido. El juez hace hasta un reintento (`juez.rs`, "Hace como mucho un reintento"). En t1 de S2, la cota dio 1,70 USD por caso. Lo medido fue de 0,08 a 0,22 USD por paso. La cota frenó una calibración que entraba.

## Tarea

La regla que manda: **la cota sigue siendo una cota, y nunca baja de lo que el juez puede gastar.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Huella de una dimensión solo de sondas

1. En una dimensión sin ítems `juez` ni `mixto`, `huella_de` junta solo sus rúbricas. No incluye `huella_del_prompt`.
2. En una dimensión con ítems del juez, la huella no cambia.
3. `huella_vigente` no cambia: una fila sin huella sirve solo en una dimensión sin juez.

**Tests en rojo primero:**
- Unitario: la huella de Seguridad no cambia si cambia `huella_del_prompt`. Simulalo con una función interna que reciba la huella del prompt como parámetro.
- Guarda: la huella de Diseño sí cambia con el prompt.
- Guarda: los tests de huella de la Tarea 81 siguen verdes.

### 2. Ventana de cambio: se recorta primero el contexto

1. Si una ventana pasa `extracto_max`, primero se achica su contexto, por igual de cada lado.
2. Si las líneas cambiadas solas pasan `extracto_max`, se parten en ventanas consecutivas del mismo archivo. Cada una respeta `extracto_max`. Todas respetan `extractos_total`.
3. La marca de recorte va entre ventanas del mismo archivo, como hoy.
4. `Extracto` guarda la última línea escrita de cada ventana, como hoy.

**Tests en rojo primero:**
- Unitario: un cambio de 20 líneas largas, con contexto que no entra, da una ventana con las 20 líneas cambiadas completas.
- Unitario: un cambio que solo ya pasa `extracto_max` da dos ventanas o más, y entre todas cubren cada línea cambiada que entra en el total.
- Guarda: los tests de ventanas de la Tarea 81 siguen verdes.

### 3. Motivo visible "calibración de otro juez"

1. `Puntaje` suma `motivo_sin_calibrar: Option<String>`, con `#[serde(default)]`.
2. Donde se aplica la calibración en `src/oracle/mod.rs`, se llena con `Calibraciones::motivo_sin_calibrar`.
3. El informe muestra el motivo junto al puntaje sin calibrar.
4. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: un puntaje de Diseño con una fila de huella vieja trae `motivo_sin_calibrar` igual a "calibración de otro juez".
- Unitario: el texto del informe muestra ese motivo.
- Guarda: un puntaje calibrado trae `None`.
- Guarda: un veredicto viejo, sin el campo, se lee con `None`.

### 4. Cota del peor caso

1. **Entrada.** Sumá en `juez.rs` una función pública que dé la cota de bytes del prompt. Sale de `LimitesPrompt`, `PROMPT_SISTEMA`, la spec, las rúbricas y la sección del reintento.
2. **Bytes, no caracteres.** Los topes de `LimitesPrompt` cuentan caracteres. Un carácter UTF-8 ocupa hasta 4 bytes. La cota usa 4 bytes por carácter. Si eso pasa de `MAX_ENTRADA_JUEZ_BYTES`, usa `MAX_ENTRADA_JUEZ_BYTES`.
3. **Pedidos.** Cada juicio cuenta 2 pedidos: el primero y el reintento. Cada pedido cuenta `INTENTOS_MAX` intentos.
4. **Salida.** Cada intento cuenta todo `max_tokens`, como hoy.
5. `peor_caso_de_caso` usa esa cota de entrada. Si un caso no conoce la spec, usa `MAX_ENTRADA_JUEZ_BYTES`, como hoy.

**Tests en rojo primero:**
- Unitario: con un juez que hace el reintento, la cota cuenta 2 pedidos. Hoy cuenta 1.
- Unitario: con una spec chica, la cota de entrada queda debajo de `MAX_ENTRADA_JUEZ_BYTES`.
- Unitario: con una spec enorme, la cota de entrada queda en `MAX_ENTRADA_JUEZ_BYTES`.
- Guarda: un modelo sin precio sigue dando cota infinita y error de configuración.
- Actualizá el test de la línea 4404 de `calibra.rs` a la fórmula nueva, y explicalo en el informe.

**Cota para t1 de S2, sin API.**
- Calculala con un test ignorado o un ejemplo, no con el CLI.
- Leé solo `~/.cache/root-wt/mineria/s2/t1/SPEC.md` y las rúbricas embebidas. No leas ni toques `ROOT_HOME`.
- Informá la cota en tres partes: bytes de entrada por intento, tokens de salida por intento, y número de intentos por caso.
- Informá también la cota vieja en las mismas partes. El orquestador la pasa a USD con los precios de `root.toml`.

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

Al terminar, escribí `docs/builds/informe-local-82.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Cota del peor caso para t1 de S2**: la vieja y la nueva, en bytes de entrada, tokens de salida e intentos.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
