# Brief de la Tarea 63 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e`. El orquestador te dice el commit. La corrida real de memora corre al mismo tiempo.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-63`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/mutacion.rs`, sus tests y `docs/interfaces.md`.
- **No toques:** el resto de `src/oracle/`, en especial `evaluar` y `src/oracle/juez.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`. El evaluador real no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La corrida real de memora corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- Una sola consulta de `cargo mutants --help` en la imagen está permitida. No corras contenedores de tests.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La calibración de memora dio especificidad 0,0 en Arquitectura y en Diseño, y 0,3 en Completitud. Los casos limpios son copias idénticas de la base, y aun así "detectan" un defecto. La regla de hoy mide el ruido: una cita nueva del juez, o un test con 84 % de pase que se mezcla. Esta tarea cambia cómo la calibración decide una detección. No cambia lo que dice una evaluación real.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-05: calibración de memora y parte de brasa".
- "Diagnóstico de la calibración (Opus, 2026-11-05)".
- "Desempates técnicos 33 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **la calibración detecta el defecto inyectado, no el ruido.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

1. **Motivo en cada caso.** `.casos.jsonl` guarda qué disparó cada detección y los niveles del juez. Un caso no aplicable también deja su renglón, con su motivo. Rojo: hoy un caso limpio detectado no dice por qué, y un no aplicable no aparece.
2. **Regla con juez.** En Estructura, Arquitectura y Diseño, un caso detecta solo por una de estas tres cosas: un hallazgo nuevo de sonda o un ítem de sonda que baja; una cita del juez a 5 líneas o menos del lugar del defecto; o el ítem del defecto que baja 2 niveles o más. Rojo: un veredicto falso con un hallazgo del juez de id nuevo, un ítem que baja 1 nivel y `valor` menor hoy detecta, y no debe detectar.
3. **Limpios simétricos.** Cada limpio usa el defecto y el lugar del caso con defecto anterior, con las mismas tres condiciones. Rojo: un limpio donde el juez cita ese lugar debe contar como falsa alarma.
4. **Tests con ruido.** En Completitud y Funciona, los tests que variaron entre la base y los limpios no cuentan para detectar. Rojo: un test estable en la base que se mezcla en un limpio hoy detecta, y no debe detectar. Un test estable en la base y en todos los limpios, que falla en un caso con defecto, sí detecta.
5. **Limpio sin veredicto.** No cuenta como detección. La fila suma `limpios_sin_veredicto`. Una dimensión queda calibrada solo si esos casos son como mucho el 20 % de los limpios y si la exactitud llega a 0,9. Un defecto sin veredicto sigue siendo un falso negativo.
6. **Limpios compartidos.** Los limpios del juez de una tarea se guardan bajo `$ROOT_HOME/calibracion/` con clave de tarea, versión del repo, hash de las rúbricas y número de caso. Las otras dos dimensiones los reusan. Rojo: calibrar Diseño después de Arquitectura con un juez falso no hace más llamadas por sus limpios.
7. **`ComportamientoRoto`.** Un defecto nuevo apunta a Funciona. Cambia un `==` por `!=` en una línea de producción, compila y hace fallar tests. Si no hay línea candidata, es no aplicable. Rojo: `calibrar --dimension funciona` hoy sale con 2.
8. **`AssertBorrado` acotado.** Un caso con `AssertBorrado` corre `cargo-mutants` solo sobre el archivo del mutante elegido. Leé `cargo mutants --help` en `root-oraculo-rust:2`. Test: el caso no corre mutantes de otro archivo, y detecta si el mutante elegido sobrevive.

No corras la calibración real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-63.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
