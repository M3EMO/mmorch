# Brief de la Tarea 116 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-117` con la 115. Trae las Tareas 84 a 115 y la 117.
- La 118 y la 119 pueden construirse a la vez. La 118 toca `src/limpieza.rs`. La 119 toca `src/oracle/eficiencia.rs`, `src/oracle/informe.rs` y el ejecutor de contenedor de las sondas. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-116`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, cuándo hay vuelta | `src/corre/corrida.rs` |
| 2, la vuelta | `src/corre/corrida.rs`, `src/corre/mod.rs` |
| 3, la fila | `src/tipos.rs` (solo `Fila`), `src/corre/corrida.rs`, `docs/interfaces.md` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/limpieza.rs`, `src/corre/contenedor.rs`, `src/corre/barrido.rs`, `src/corre/credencial.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- No lances `claude` real. Los tests usan el ejecutor falso de `corre`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario sobre P106, literal:** "b Y c". La opción "b" decía: el informe nombra la medida que más empeoró, y en `r00t corre` el builder recibe una vuelta para mejorarla.

**Lo que ya existe.** La Tarea 115 suma `Veredicto.eficiencia_auto`. Por cada medida lleva la referencia, el repo, la razón, el ruido, si hay regresión, y el motivo si quedó sin valor. La regla de "no listo" por más de 1,5 veces vive en `src/oracle/informe.rs`.

**Lo que pasa hoy en `corre`.** `correr_interno` (`src/corre/corrida.rs:153`) lanza los roles en orden (paso e). Después revisa los transcripts (paso g), quita la copia de la credencial (paso h, `guardia.cerrar`), arma el estado final y evalúa (paso i). La fila lleva esa evaluación. Nada vuelve al builder después de evaluar.

**El obstáculo.** La credencial ya no está cuando llega el veredicto. Una vuelta más necesita la credencial otra vez, con su guardia, como en el paso e.

## Tarea

La regla que manda: **si el cambio empeora la eficiencia por encima del ruido, el builder recibe una sola vuelta más para mejorarla, y cuenta la evaluación de después.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Cuándo hay vuelta

1. **Una función pura** recibe el veredicto y dice si hay vuelta. Hay vuelta si alguna medida de `eficiencia_auto` trae regresión.
2. **No hay vuelta** si la corrida no terminó bien: un rol salió con otro código, agotó sus vueltas cortadas, o se quedó sin cupo.
3. **No hay vuelta** si la evaluación falló, o si `eficiencia_auto` viene vacío.
4. **No hay vuelta** si la decisión es "bloqueado". Un bloqueo no es de eficiencia.

**Tests en rojo primero:**
- Unitario: una regresión de 1,3 veces por encima del ruido da vuelta.
- Unitario: una medida sin regresión no da vuelta.
- Unitario: un veredicto sin `eficiencia_auto` no da vuelta.
- Unitario: un veredicto "bloqueado" con una regresión no da vuelta.
- Unitario: una corrida con un rol que salió con 1 no da vuelta.

### 2. La vuelta

1. **El rol** es el último rol de la configuración que salió con 0. Lleva `--resume` con su última sesión.
2. **Antes de lanzar,** el rol vuelve a rutear por cupo (`rutear_por_cupo`), como cada vuelta. Se espera la vigencia del login, como en el paso e.
3. **La credencial.** Buscá dónde se arma la copia de la credencial para un lanzamiento. Durante la vuelta, la copia existe con su propia `GuardiaCredencial`. Al terminar la vuelta, la guardia la quita, en todo camino de salida.
4. **La instrucción** empieza con una constante nueva, `PROMPT_EFICIENCIA`. Nombra cada medida con regresión, su razón y los dos números. Pide bajarla sin cambiar el comportamiento, y hacer commit. Después va el prompt del rol.
5. **Un corte por plazo** sigue la regla de `MAX_VUELTAS`, como el paso e.
6. **Después de la vuelta:** se revisa su transcript con las huellas del oráculo, como el paso g. Se arma otro estado final, en otra carpeta de la corrida. Se evalúa otra vez.
7. **La segunda evaluación es la que cuenta.**
8. **Una sola vuelta.** La segunda evaluación no da otra vuelta, aunque la regresión siga.
9. **Sin cupo, sin vigencia o con un rol que no arranca:** la corrida no falla. La fila lleva la primera evaluación, y un `gate` anota el motivo.
10. **Reutilizá el código del paso e.** Si hace falta, sacá el lanzamiento de un rol a una función, y usala en los dos lugares. No dupliques el bucle.

**Tests en rojo primero, con el ejecutor y el evaluador falsos:**
- Unitario: un evaluador que da una regresión y después ninguna lanza dos veces el último rol, y evalúa dos veces. La fila lleva la segunda evaluación.
- Unitario: la instrucción de la vuelta nombra la medida, la razón y los dos números.
- Unitario: un evaluador que siempre da una regresión evalúa dos veces, no tres.
- Unitario: sin cupo para la vuelta, la fila lleva la primera evaluación y el `gate` nombra el motivo.
- Unitario: la copia de la credencial no existe al terminar la corrida, también si la vuelta falla.
- Guarda: una corrida sin regresión lanza y evalúa una sola vez, como hoy.
- Guarda: los tests de `corre` de las Tareas 84 a 112 siguen verdes.

### 3. La fila

1. **`Fila` suma `vuelta_eficiencia`,** con `#[serde(default)]`. Lleva la primera regresión: cada medida, su razón y los dos números. También lleva si la vuelta corrió, y el motivo si no corrió.
2. **`vueltas`** cuenta también el lanzamiento de la vuelta de eficiencia.
3. Una fila vieja sin el campo se lee.
4. Regenerá `docs/interfaces.md`.

**Tests en rojo primero:**
- Unitario: una corrida con vuelta deja `vuelta_eficiencia` con la primera regresión, en la fila y en `runs.jsonl`.
- Unitario: una fila vieja de `runs.jsonl`, sin el campo, se lee.

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

Al terminar, escribí `docs/builds/informe-local-116.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La credencial en la vuelta**: dónde se arma la copia, qué guardia la cubre, y cómo lo prueban los tests.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
