# Brief de la Tarea 90 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-89` (af2ab61). Las Tareas 84 a 89 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-90`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, build y escenarios | `src/oracle/entrega.rs` |
| 2, respuestas incompletas | `src/proveedores/deepseek.rs`, `src/proveedores/anthropic.rs` |
| 3, redactor | `src/oracle/informe.rs` |
| 4, eventos con lock | `src/aprende.rs` |
| 5, origen de eventos por MCP | `src/mcp.rs`, `src/observe/mod.rs`, `docs/interfaces.md` |
| 6, inestables con `tarea_hash` | `src/oracle/calibra.rs`, `src/oracle/mod.rs` |
| 7, visibilidad no es arista | `src/oracle/perfiles/rust.rs` |

- Los siete puntos no comparten archivos. Corren en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`, no `codegraph.cmd`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

La revisión de cierre del hito 1 dejó MEDIUM abiertos. El orquestador los verificó sobre 76f95c6. Root evalúa código que escribe un builder, que es un modelo. Un builder puede dañar el host o aprender a engañar al evaluador. Esta tarea cierra seis de esos MEDIUM y dos detalles de la Tarea 89.

- **MEDIUM 11.** Los escenarios de Entrega corren con `podman run -d` sin `--cap-drop=all`, sin `--security-opt=no-new-privileges`, sin `--pids-limit` y sin `--memory` (`src/oracle/entrega.rs`, `args` de escenario cerca de la línea 1199). El build tampoco los lleva (`args_build`, línea 177). Los contenedores de `src/oracle/contenedor.rs:116` a `118` ya los llevan.
- **MEDIUM 14.** Nada impide que un `Dockerfile` del repo pida `RUN --network=host`. El build corre con `--network none` o con `pasta`, pero no se probó si un `RUN --network` lo pasa por alto.
- **MEDIUM 9.** `deepseek.rs:88` y `anthropic.rs:115` marcan como truncada solo una respuesta cortada por tope. Una respuesta con `content_filter` o `refusal` pasa como completa.
- **MEDIUM 17.** `aplicar_redaccion` (`src/oracle/informe.rs:749`) junta con `zip` los índices visibles y la lista del redactor. Si el redactor devuelve otra cantidad, cada texto cae en la acción equivocada.
- **MEDIUM 21.** `leer_eventos` de `src/aprende.rs:546` no toma el lock común. Una rotación en paralelo pierde filas en silencio. `observe::uso::leer_eventos` (`src/observe/uso.rs:201`) sí lo toma.
- **MEDIUM 22.** `root_registrar` por MCP puede crear eventos `humano` y `plan-aprobado` (`src/mcp.rs:240`). El evento no dice que vino por MCP, y el resumen los cuenta como de una persona.
- **Tarea 89, pregunta 2.** `inestables_de` usa la última fila con huella vigente, sin mirar `tarea_hash`. Una fila de otro `tarea.toml` puede excluir ítems.
- **Tarea 89, detalle.** `pub(in crate::x) use ...` cuenta `x` como destino. Un camino de visibilidad no es una dependencia, y una arista falsa puede inventar un ciclo.

## Tarea

La regla que manda: **el código evaluado no gana privilegios, y nada que escriba un agente pasa por una decisión humana.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Build y escenarios

1. **Escenarios.** El `podman run -d` de un escenario suma `--cap-drop=all`, `--security-opt=no-new-privileges`, `--pids-limit` y `--memory`. Usá las mismas constantes que `src/oracle/contenedor.rs`. Si no son visibles desde `entrega.rs`, hacelas `pub(crate)` sin moverlas, y anotalo en el informe.
2. **Build.** `args_build` suma las mismas opciones que `podman build` acepte en la versión de la imagen. Revisá `podman build --help`. Una opción que `podman build` no acepta no se agrega, y se anota en el informe.
3. **`RUN --network`.** Antes del build, Root lee el `Dockerfile` elegido. Una instrucción `RUN` con `--network` frena el build. El resultado es rojo, con el motivo "el Dockerfile pide red en RUN". El chequeo tolera mayúsculas, espacios y la forma `--network host`.

**Tests en rojo primero:**
- Unitario: los argumentos de un escenario traen las cuatro opciones.
- Unitario: los argumentos del build traen las opciones que acepta `podman build`.
- Unitario: un `Dockerfile` con `RUN --network=host apt-get update` da rojo con su motivo, sin llamar a podman.
- Unitario: `run --network host` en minúsculas y con espacio también frena.
- Guarda: un `Dockerfile` sin `--network` construye como hoy.
- Guarda con podman: un escenario que intenta `ping` o crear un proceso de más falla dentro del contenedor. Usá `crate::saltar_sin_podman!();`.

### 2. Respuestas incompletas

1. En `anthropic.rs`, un `stop_reason` igual a `refusal` marca la respuesta como incompleta, igual que `max_tokens`.
2. En `deepseek.rs`, un `finish_reason` igual a `content_filter` marca la respuesta como incompleta, igual que `length`.
3. Usá el mismo campo que hoy marca la respuesta truncada. Si conviene distinguir el motivo, sumá un campo con `#[serde(default)]` y anotalo en el informe.

**Tests en rojo primero:**
- Unitario: un stream de Anthropic con `stop_reason` igual a `refusal` da respuesta incompleta.
- Unitario: una respuesta de DeepSeek con `finish_reason` igual a `content_filter` da respuesta incompleta.
- Guarda: `end_turn` y `stop` siguen siendo completas.

### 3. Redactor

1. Si la lista del redactor no tiene la misma cantidad que los índices visibles, esa lista no se aplica. El informe conserva sus textos originales.
2. El informe suma un aviso: "el redactor devolvió N textos para M acciones; se conservan los originales".
3. Con la misma cantidad, todo sigue igual.

**Tests en rojo primero:**
- Unitario: un redactor que devuelve 2 riesgos para 3 visibles deja los 3 textos originales y el aviso.
- Guarda: un redactor con la misma cantidad aplica sus textos, como hoy.

### 4. Eventos con lock

1. `aprende.rs` lee los eventos con `observe::uso::leer_eventos`, que toma el lock común.
2. Si `aprende.rs` necesita convertir el error, convertilo sin cambiar el mensaje.
3. Borrá la función local si queda sin uso, después de unir sus llamadores.

**Tests en rojo primero:**
- Unitario: con el lock tomado por otro hilo, `aprende` espera y lee todas las filas.
- Guarda: los tests de `aprende` siguen verdes.

### 5. Origen de los eventos por MCP

1. Todo evento que crea `root_registrar` por MCP lleva `origen: "mcp"`. El campo nuevo lleva `#[serde(default)]`.
2. `resumir_eventos` no cuenta como humanos ni como planes aprobados los eventos con `origen` igual a `"mcp"`. Los cuenta aparte, como "declarados por MCP".
3. Un evento viejo sin `origen` se cuenta como hoy.
4. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: `root_registrar` con tipo `humano` escribe `origen: "mcp"`.
- Unitario: el resumen no suma ese evento a los humanos, y lo muestra como declarado por MCP.
- Guarda: un evento `humano` sin `origen` cuenta como humano.

### 6. Inestables con `tarea_hash`

1. `inestables_de` usa solo una fila con huella vigente y con el mismo `tarea_hash` actual, o sin `tarea_hash`.
2. Si hace falta propagar un `Result` por `medir`, hacelo, y anotá los llamadores revisados.

**Tests en rojo primero:**
- Unitario: una fila con otro `tarea_hash` no excluye sus inestables.
- Guarda: una fila con el mismo `tarea_hash` los excluye, como hoy.

### 7. Visibilidad no es arista

1. En una sentencia `use` con visibilidad `pub(in crate::x)`, el camino de la visibilidad no aporta destino.
2. El camino importado sí aporta su destino, como hoy.

**Tests en rojo primero:**
- Unitario: `pub(in crate::a) use crate::b::X;` da solo la arista hacia `b`.
- Guarda: la medición sobre broot de la Tarea 89 da las mismas 219 aristas o menos, sin aristas que nombren módulos solo desde una visibilidad.

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

Al terminar, escribí `docs/builds/informe-local-90.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Opciones de `podman build`**: cuáles acepta la versión de la imagen y cuáles no.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
