# Brief de la Tarea 78 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e`. El orquestador te dice el commit.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-78`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/auditar.rs`, `src/oracle/validez.rs`, `src/oracle/perfiles/herramientas.rs`, `src/oracle/juez.rs` (solo extractos y secretos), `src/oracle/evidencia.rs` (solo exponer `ruta_dentro`), sus tests y `docs/interfaces.md`.
- **No toques:** `PROMPT_SISTEMA`, `PROMPT_AUDITORIA`, `src/oracle/calibra/`, `src/corre/`, `src/route/`, `src/proveedores/` y `$ROOT_HOME/`. En un repo sin secretos ni enlaces, un veredicto no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La minería corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Una revisión halló fugas de secretos y sesgos en la medición de `auditar` y `validar`. Esto va antes de medir el conjunto final y de evaluar repos ajenos.

Leé en el plan: "Vuelta del 2026-11-07 (1)", parte de la revisión.

## Tarea

La regla que manda: **un secreto no sale, y una medición cuenta lo que pasó.** Hacé un commit por punto. Cada punto empieza con su test en rojo, con datos de la forma real. Ningún test usa una API ni un secreto real.

1. **Rango de gitleaks.** `FugaGitleaks` lee `EndLine`. Sin ese campo, usa `StartLine`. `juez` y `auditar` omiten todo el rango. Rojo: un reporte de las líneas 10 a 20 hoy deja salir la línea 12.
2. **Ruta de gitleaks.** Una ruta se normaliza contra el repo: `./`, `/repo/`, `/salida/copia/` y la ruta absoluta del repo. Una ruta absoluta fuera del repo da error de uso. Un hallazgo cuyo archivo no existe en el repo deja un aviso. Rojo: una ruta absoluta del repo hoy no omite nada, sin aviso.
3. **`--solo-medir` sin texto.** Todo descarte va sin `cita` ni `detalle`. Un hallazgo en un archivo con líneas omitidas queda con `detalle` vacío. Rojo: "campos incompletos" y "el archivo no se envió" hoy copian el texto, y un `detalle` con el secreto entra en `hallazgos`.
4. **Línea visible.** Un defecto cuenta como `medido` solo si su línea está en el texto visible. Una línea omitida o después del corte da `medido = false`. Rojo: un defecto después de un `#[cfg(test)]` a mitad del archivo hoy cuenta como no encontrado.
5. **Uno a uno.** Un candidato cubre un solo defecto. Primero se emparejan las distancias menores, y un empate va por el orden del archivo. Rojo: dos defectos a 4 líneas, y un candidato en el medio, hoy dan 2.
6. **Rojo estable.** `test_rojo` cuenta `ocultos_fallidos` y `contrato_fallidos`. El detalle suma `test_inestable`, que no cuenta para `recall`. Con una sonda sin veredicto, el detalle suma `sonda_sin_veredicto`. Ese defecto sigue en el denominador. Rojo: un test en `inestables` hoy cuenta como rojo.
7. **Salida con tope.** `leer_salida` usa el mismo tope y el rechazo de enlaces de `leer_de_salida`. Un archivo de más del tope da sin veredicto. Rojo: hoy lee sin tope.
8. **Ruta del juez.** `extractos` usa `ruta_dentro`. Un archivo con `..` que sale del repo no se lee. Rojo: un hallazgo con `../afuera.txt` hoy manda ese archivo al prompt.

**Guardas.** `huella_del_prompt` y `hash_prompt` no cambian. Un veredicto viejo se lee igual.

No corras la validez, la corrida ni la pasada reales. Las corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-78.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
