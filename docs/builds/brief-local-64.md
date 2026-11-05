# Brief de la Tarea 64 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-63-p5` (e575fd0). No esperes la mezcla de la Tarea 62.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-64`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, `docs/interfaces.md` y sus tests. En `src/oracle/mod.rs`, solo `Partes::mide`, su comentario y sus tests. En `src/oracle/juez.rs`, solo una función que da la huella del prompt.
- **No toques:** `src/oracle/mutacion.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`. Una evaluación real, con `Partes::todas`, no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La corrida real de memora corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La revisión de la Tarea 63 encontró dos fallas que invalidan la medición. Los tests no las ven, porque sus evaluadores falsos no se comportan como `evaluar`. La medición cuesta plata real, así que se corrigen antes.

Leé en el plan, antes de empezar:

- "Revisión del código de la Tarea 63 (Opus, sobre e575fd0)".
- "Desempates técnicos 34 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un test de la calibración se comporta como el evaluador real.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

1. **Tres dimensiones en una evaluación.** `Partes::mide` acepta Estructura, Arquitectura y Diseño cuando la dimensión calibrada es una de ellas y `juez_diseno` está prendido. Rojo: un `evaluar` real con juez falso y `partes_de(Arquitectura)` hoy deja Diseño y Estructura con "no medida en esta calibración". Después, las tres deben tener veredicto, con una sola llamada al juez.
2. **Lectura segura del cache.** `Compartidos::leer` rechaza un puntaje cuyo `sin_veredicto` es `NO_MEDIDA`. Un `sin_veredicto` por cobertura se reusa. Rojo: un evaluador falso que devuelve `NO_MEDIDA` en Diseño, como el real, hoy reusa ese limpio. Debe evaluar otra vez.
3. **Prueba de punta a punta.** Calibrá Arquitectura y Diseño con `evaluar` real y juez falso. Rojo: Diseño debe dar 0 llamadas por sus limpios, y `limpios_sin_veredicto` debe dar 0. Hoy da el total de los limpios.
4. **Clave completa.** La clave suma el modelo elegido para `juzga`, el hash del prompt del juez, el hash de `tarea.toml` y la semilla. Rojo, un test por campo: cambiar el campo no reusa nada. Sin `root.toml`, como en los tests, el modelo vale `sin_juez`.
5. **`limpio_reusado`.** Cada renglón de `.casos.jsonl` dice si el limpio salió del cache. El campo lleva `#[serde(default)]`. Rojo: un limpio reusado hoy no se distingue.
6. **Pruebas comparable.** Un caso con `AssertBorrado` detecta solo si un mutante que la base mató aparece vivo en el caso. Compará `archivo` y `cita` del hallazgo contra los muertos de la base. El `valor` y los otros ids no cuentan en ese caso. El caso guarda el motivo `mutante_vivo:<nombre>`. Tres tests en rojo:
   - un `valor` menor, sin mutante de la base vivo, hoy detecta y no debe detectar;
   - un mutante vivo fuera de los muertos de la base hoy detecta y no debe detectar;
   - un mutante de la base vivo, con `valor` mayor que la base, debe detectar.
7. **`detecta_item` con la regla nueva.** En Estructura, Arquitectura y Diseño, un ítem cuenta solo si baja 2 niveles o más. Un limpio sin nivel no cuenta como falsa alarma. Rojo: una baja de 1 nivel hoy cuenta.
8. **Guarda del hilo local.** Un test corre `evaluar` real dentro de `con_archivo_acotado`, con un ejecutor que anota comandos. La corrida debe llevar `--file`. Es una guarda: no hace falta verla en rojo, y el informe lo dice.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-64.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
