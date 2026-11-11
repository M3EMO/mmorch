# Brief de la Tarea 91 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Base: punta de laptop/tarea-90. El orquestador completa el commit: `________`.
- Las Tareas 84 a 90 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-91`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, clasificación del build | `src/oracle/entrega.rs` |
| 2, escenarios por `podman exec` | `src/oracle/entrega.rs`, `src/oracle/contenedor.rs` |
| 3, junit con el contenedor muerto | `src/oracle/mod.rs` |

- Los puntos 1 y 2 comparten `entrega.rs`. Van en serie: primero el 1.
- El punto 3 corre en paralelo.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

Root evalúa código que escribe un builder, que es un modelo. Un modelo optimizado contra Root puede encontrar atajos que falsifican el resultado, sin intención maliciosa. El daño es una decisión "listo" falsa. La revisión de cierre dejó dos MEDIUM de esta clase, el 15 y el 12. El diseño está en la vuelta del 2026-11-11 (12) del plan.

**Las tres reglas del diseño:**
1. Un resultado viaja por el código de salida de un proceso que Root lanzó. Nunca por un archivo que el código evaluado puede escribir.
2. Un archivo que escribe el código evaluado se lee recién con el contenedor muerto, y se cruza con un código de salida de la regla 1.
3. Un texto que imprime el repo nunca decide la clasificación. Solo cuenta el texto que imprime podman después de que terminó el proceso del repo.

**MEDIUM 12, clasificación del build.** `clasificar_fallo` (`src/oracle/entrega.rs:633`) busca la última línea `STEP` en toda la salida del build. La salida de un `RUN` es texto que imprime el repo. El repo puede imprimir una línea `STEP` falsa y mover el paso que falló. También puede imprimir "could not resolve host" y pasar de rojo a `PendienteRed`. Ante un `PendienteRed`, Root repite el build con red (`entrega.rs:431`). Si falla otra vez y el texto vuelve a parecer de red, queda pendiente con `AVISO_RED_TAMPOCO`. Un `RUN` que falla siempre e imprime ese texto nunca da rojo.

**MEDIUM 15, escenarios en el contenedor de evaluación.** `construir_script_con` (`entrega.rs:1259`) arma un solo script de `sh`. El script lanza `correr` en segundo plano y después corre los escenarios. Cada escenario escribe su código en `/salida/entrega-escenario-<id>.rc`. `/salida` es escribible, y el producto corre con el mismo usuario. Un proceso del producto puede escribir `0` en cada `.rc`. Lo mismo vale para `entrega-instalar.rc`, `entrega-correr.rc`, el marcador `.fin` y el marcador de arranque. El camino de la imagen construida ya usa `podman exec` (`ContenedorEscenario`, `entrega.rs:1074`), y ese camino no se puede falsificar.

**Junit.** `juzgar_junit` (`src/oracle/mod.rs:2341`) ya cruza el junit con el código del runner. Falta comprobar que Root lo lee con el contenedor del runner muerto.

## Tarea

La regla que manda: **ningún resultado pasa por un lugar que el código evaluado puede escribir mientras está vivo.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Clasificación del build (MEDIUM 12)

1. **Paso que falló.** Sale de la última línea no vacía del stderr, si tiene la forma `Error: building at STEP "<instrucción>": <causa>`. Compará la instrucción con el `Dockerfile`, como hoy con `indice_del_paso`.
2. **Las líneas `STEP` del medio** y el campo `SalidaPodman.paso` ya no deciden. Pueden quedar en la cita como contexto.
3. **Sin esa línea final,** el build queda sin veredicto, con el motivo "podman no informó el paso que falló". No es `Infra` si hubo líneas `STEP`.
4. **Imagen base ausente.** `PATRONES_AUSENTE` se busca solo en la causa de la línea final.
5. **Con red disponible, un fallo es rojo.** Si el build con red falla en un paso, el texto de red ya no cuenta. El resultado es "la imagen no construye". `AVISO_RED_TAMPOCO` queda solo si `red_no_configurada` reconoce el error, que es un texto de podman.
6. **Primer build sin red.** `pide_red` sigue pudiendo dar `PendienteRed`. Ese pendiente siempre se resuelve con el build con red.

**Tests en rojo primero:**
- Unitario: un `RUN` que imprime `STEP 9/9: FROM falsa` y falla da el paso real de la línea final.
- Unitario: un `RUN` que imprime "could not resolve host" y falla sin red da `PendienteRed`. Con el build con red fallando igual en el mismo paso, da rojo con "la imagen no construye".
- Unitario: un stderr sin línea final de podman y con líneas `STEP` da sin veredicto, con su motivo.
- Unitario: una imagen base ausente sigue dando su pendiente cuando la causa de la línea final lo dice.
- Guarda: la salida real de la línea 2953 del test de hoy, el caso de slirp4netns, sigue dando lo mismo.
- Guarda con podman: un `Dockerfile` cuyo `RUN` imprime una línea `STEP` falsa y sale con 1 da rojo en el paso correcto. Usá `crate::saltar_sin_podman!();`.

### 2. Escenarios por `podman exec` (MEDIUM 15)

1. **Arranque.** El contenedor de evaluación arranca separado, con un proceso que lo mantiene vivo, como `ContenedorEscenario::iniciar`. Lleva los mismos montajes, el mismo usuario y el endurecimiento de la Tarea 90.
2. **Un `podman exec` por paso.** El preludio, la copia de `/repo` a la carpeta de trabajo, `instalar` y cada escenario corren cada uno con su propio `podman exec`.
3. **Código por proceso.** El resultado de cada paso es el código de salida de su `podman exec`. Los archivos `.rc` y el marcador de arranque desaparecen.
4. **`correr` como hijo.** `correr` es un `podman exec` que Root mantiene como proceso hijo. Después de `espera_segundos`, Root mira ese hijo con `try_wait`. Si terminó, `correr` murió. Si sigue, está vivo. El marcador `.fin` desaparece.
5. **Logs.** Los logs de cada paso salen del stdout y stderr del `podman exec`, con el lector con tope de la Tarea 85. Las líneas que hoy entran a `/salida` siguen entrando al veredicto, con los mismos topes.
6. **Final.** Root termina el hijo de `correr` y quita el contenedor, aunque un paso falle.
7. **Una sola función de escenario.** El camino de la imagen y el de evaluación usan la misma función para correr un escenario.
8. **Infraestructura.** Los motivos de `fallo_de_infraestructura` siguen existiendo, ahora según el código de cada `podman exec`. Un `podman exec` que no pudo arrancar es infraestructura, no un rojo del producto.

**Tests en rojo primero:**
- Unitario: con un ejecutor falso, un producto que escribe `0` en `/salida/entrega-escenario-a.rc` no cambia el resultado del escenario `a`, que sale del código de su `podman exec`.
- Unitario: `correr` que termina antes de `espera_segundos` da "correr no quedó vivo", sin leer ningún archivo.
- Unitario: `correr` vivo a los `espera_segundos` da "correr quedó vivo".
- Unitario: un fallo de `instalar` corta los escenarios y termina el hijo de `correr`.
- Guarda: los tests de Entrega de hoy dan los mismos estados, adaptados a los códigos de proceso.
- Guarda con podman: un producto con un proceso de fondo que reescribe cada `.rc` con `0` no hace pasar un escenario que falla. Usá `crate::saltar_sin_podman!();`.

### 3. Junit con el contenedor muerto

1. Verificá que el `podman run` de la suite termina antes de que Root lea el junit en `juzgar_junit`.
2. Si ya es así, sumá un test de guarda que lo fije, y explicá en el informe dónde se espera el fin del contenedor.
3. Si no es así, Root espera el fin del contenedor, o lo quita, antes de leer.

**Test:**
- Guarda: con un ejecutor falso que anota el orden, el fin del contenedor ocurre antes de la lectura del junit.

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

Al terminar, escribí `docs/builds/informe-local-91.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, todo con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Archivos de `/salida` que quedan**: cuáles escribe todavía el contenedor y quién los lee.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
