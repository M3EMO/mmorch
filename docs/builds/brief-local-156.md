# Brief de la Tarea 156 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (68) y (69).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Esta es la segunda versión del brief.** La primera terminó sin código, en `laptop/tarea-156` (8570e9e). Su informe propuso tres opciones, y el planner eligió la A.
- Arrancá desde la punta de `laptop/tarea-156`. Trae la cadena 110 a 155 y el informe de la primera versión.
- Ningún otro builder corre a la vez. **No corras mientras el orquestador use podman** para `esbozos.py`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en el mismo worktree y en la misma rama: `laptop/tarea-156`. Preguntale al orquestador si cambió.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la carga ajena como número en el caso | `src/oracle/eficiencia.rs`, `src/oracle/calibra.rs` |
| 1, si el número cruza `Veredicto` | además, el archivo donde vive el struct que lleva las medidas de Eficiencia al veredicto |

- **Antes de tocar un tercer archivo,** anotá en el informe cuál es y por qué. Solo ese archivo, y solo para llevar el número.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **El esquema cambia solo por adición.** Cada campo nuevo es `Option<…>` con `#[serde(default)]`. Una fila vieja se lee igual, con el campo vacío.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` muestran `CasoGuardado` o `Veredicto`, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita otro archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La v6 de Eficiencia corrió en Colab.** En t1 y t3, casi todos los casos perdieron sus tiempos con "la máquina tenía carga ajena: 1.00 núcleos de 2". En t2, no. Para revisar el umbral hace falta el valor de cada caso, y hoy ese valor se pierde.

**Dónde se pierde, según el informe de la primera versión:**
- `Vigilancia::aviso` arma la línea "eficiencia: mayor carga ajena de una corrida: X núcleos de N".
- `medir_contra_referencia` la suma a `avisos`.
- `oracle/mod.rs` la pasa a `Veredicto.avisos`.
- `CasoGuardado::de`, en `src/oracle/calibra.rs`, no copia `veredicto.avisos`. El valor se pierde ahí.

**Por qué la A.** El planner eligió un campo numérico, `carga_ajena: Option<f64>`.
- El umbral se revisa con números, y un número no pide parseo.
- Una lista de avisos en texto, la B, mezcla avisos de distinto tipo y obliga a parsear.
- Reusar `MedidaEficiencia.motivo`, la C, rompe su regla: hoy, sin motivo, la medida tiene valor.

**El punto 1 de la primera versión no vuelve.** La Prueba 0f mostró 2 CPU visibles y 2 usables en Colab. La causa probable de la carga ajena es el robo de CPU del hipervisor. La Prueba 0g del orquestador lo va a medir. No pide código en esta tarea.

## Tarea

La regla que manda: **el caso guarda su mayor carga ajena como número, y el número viaja por la estructura, no por un texto.** Hacé un commit. Empieza con su test en rojo.

### 1. La carga ajena como número en el caso

1. **`CasoGuardado` suma `carga_ajena: Option<f64>`,** con `#[serde(default)]`. Es la mayor carga ajena de las corridas medidas del caso, en núcleos.
2. **El número viaja por la estructura.** Seguí el camino que ya lleva las medidas de Eficiencia desde `medir` hasta `CasoGuardado::de`. No parsees el texto de `Veredicto.avisos`.
3. **Si el camino cruza `Veredicto`,** sumá ahí un campo opcional, también con `#[serde(default)]`. Elegí el struct que ya lleva las medidas de Eficiencia, si existe.
4. **Un caso de otra dimensión** deja `carga_ajena` vacío.
5. **Un caso de Eficiencia sin control de carga,** por ejemplo sin fuente o sin `ROOT_CPU_US`, también lo deja vacío. Nunca se escribe un cero supuesto.
6. **El aviso de texto en `Veredicto.avisos` no cambia.**

**Tests en rojo primero:**
- Unitario: un caso de Eficiencia con una carga ajena de 0,30 núcleos guarda 0,30 en `carga_ajena`. Hoy no guarda nada.
- Unitario: un caso de Eficiencia sin control de carga deja `carga_ajena` vacío.
- Guarda: un caso de otra dimensión deja `carga_ajena` vacío.
- Guarda: un renglón viejo, sin el campo, se lee sin error.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 156.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, reemplazá `docs/builds/informe-local-156.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El camino del número**: desde `Vigilancia` hasta `CasoGuardado`, struct por struct, y cada campo nuevo.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
