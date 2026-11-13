# Brief de la Tarea 113 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-110`, en 741a43d. Trae las Tareas 84 a 110.
- La 111 está en `laptop/tarea-111` y toca `src/oracle/calibra.rs`. La 112 se construye en `laptop/tarea-112` y toca `src/limpieza.rs` y `src/corre/barrido.rs`. La 117 toca las copias con `cp`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-113`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, todo test rojo da "no listo" | `src/oracle/informe.rs` |
| 2, una dimensión inestable cuenta como medida | `src/oracle/informe.rs`, y en `src/oracle/calibra.rs` solo una función pública junto a `PREFIJO_INESTABLE` |

- Los dos puntos tocan `informe.rs`. Van en serie.
- **No toques:** `src/tipos.rs`, `src/oracle/mod.rs`, el resto de `src/oracle/calibra.rs`, `src/lib.rs`, el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario sobre P95, literal:** "p95 por que no detecta el test rojo? la idea no es solo de funcionamiento si no de eficiencia. Si hay algo que haga mas lento de lo que deberia, mas pesado de lo que deberia u otra cosa entonces deberia dar mal o una escala continua." Esta tarea cubre el test rojo. La eficiencia va en la Tarea 115.

**Lo que pasa hoy con un test rojo.** `no_listo` (`src/oracle/informe.rs:499`) solo dispara por un test que falla si ningún requisito lo prueba. Lo hacen `tests_fuera_de_requisitos` (línea 377) y `tests_contrato_fuera_de_requisitos` (línea 395). Un test que falla y que un requisito cubre solo baja la completitud. Con `completitud_minima` en 0,9, un test rojo entre muchos verdes da "listo".

**Las dos listas ya excluyen los inestables.** `v.ocultos_fallidos` y `v.contrato_fallidos` salen de `fallidos_de` (`src/oracle/mod.rs:982`). Esa función solo lee los tests estables que fallan.

**La decisión del usuario sobre P104, literal:** "b". La opción decía: "Una dimensión inestable cuenta como medida, y el resumen la nombra."

**Lo que pasa hoy con una dimensión inestable.** `aplicar_inestables` (`src/oracle/mod.rs:1748`) deja la dimensión con `sin_veredicto`, y su motivo empieza con `PREFIJO_INESTABLE`, "inestable en esta tarea" (`src/oracle/calibra.rs:286`). La Tarea 110 hizo que `sin_decision` dispare por todo `sin_veredicto`. `sin_veredicto_de` (`src/oracle/informe.rs:326`) es el único lugar de esa regla, y su comentario nombra P104.

**Por qué el prefijo, y no `Puntaje.inestables`.** Una dimensión puede traer `inestables` y estar sin veredicto por otra causa, como un juez que no corrió. Solo el motivo distingue los dos casos.

## Tarea

La regla que manda: **un test estable que falla da "no listo". Una dimensión inestable no frena la decisión, y el resumen la nombra.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Todo test rojo da "no listo"

1. **Regla nueva:** `test_oculto_falla: <id>`. Dispara por cada id de `v.ocultos_fallidos`, lo cubra o no un requisito.
2. **Su frase** nombra el test y los requisitos que lo prueban. Por ejemplo: "El test oculto t3 falla; lo prueba el requisito R2." Sin requisito, dice "y ningún requisito lo prueba", como hoy.
3. **Lo mismo para el contrato:** `test_contrato_falla: <id>`, sobre `v.contrato_fallidos`. Un requisito lo prueba con el prefijo `contrato:`, como hoy.
4. Las reglas `test_oculto_fuera_de_requisitos` y `test_contrato_fuera_de_requisitos` quedan reemplazadas. Borrá las funciones que queden sin uso, después de unir sus llamadores.
5. **Una sola función** da los requisitos que prueban un test. Usa `cubre`, como hoy.
6. La regla de completitud bajo su mínimo sigue igual.
7. El orden de `decidir` no cambia: bloqueos, "no listo", "sin decisión" y "listo".

**Tests en rojo primero:**
- Unitario: un veredicto con un test oculto que falla, cubierto por un requisito, y completitud 0,95 da "no listo". Hoy da "listo".
- Unitario: la frase de ese disparo nombra el test y el requisito.
- Unitario: un test del contrato que falla, cubierto con `contrato:`, da "no listo".
- Unitario: un test inestable que no está en `ocultos_fallidos` no dispara.
- Guarda: un test que falla sin requisito sigue dando "no listo".
- Guarda: un veredicto con un secreto y un test rojo da "bloqueado".

### 2. Una dimensión inestable cuenta como medida

1. **En `calibra.rs`,** sumá `pub fn es_motivo_inestable(motivo: &str) -> bool`, junto a `PREFIJO_INESTABLE`. No toques nada más de ese archivo.
2. **`sin_veredicto_de`** devuelve `None` si el motivo es de inestable. Actualizá su comentario: la regla sigue P104 b.
3. **Sin valor no hay mínimo.** Una dimensión inestable no trae valor medido. Las reglas de mínimo siguen mirando `sin_veredicto.is_none()`, así que no la evalúan. No cambies eso.
4. **El resumen** suma una línea: "Inestable en esta tarea: <dimensiones>." Usa la misma forma para una y para varias, como "Sin medir:".
5. **"Sin medir:"** deja de nombrar una dimensión inestable.
6. **Dimensiones que bloquean y Completitud.** Si Seguridad o Completitud quedan inestables, siguen dando "sin decisión". P104 b no cubre una dimensión que bloquea. Anotalo en el informe si algún test lo toca.

**Tests en rojo primero:**
- Unitario: el veredicto de `informe_diseno_inestable_en_la_tarea_da_sin_decision`, con Diseño inestable y todo lo demás en verde, da "listo". Renombrá ese test.
- Unitario: el resumen de ese veredicto dice "Inestable en esta tarea: Diseño." y no dice "Sin medir: Diseño.".
- Unitario: Diseño inestable y Arquitectura sin juez dan "sin decisión", con `dimension_sin_veredicto: arquitectura`.
- Unitario: `es_motivo_inestable` da verdadero con "inestable en esta tarea: diseno.errores" y falso con "el juez no corrió".
- Guarda: Diseño sin juez sigue dando "sin decisión".

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

Al terminar, escribí `docs/builds/informe-local-113.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Reglas reemplazadas**: cada regla vieja, la nueva, y cada test que leía el nombre viejo.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
