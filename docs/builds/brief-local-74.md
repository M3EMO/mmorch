# Brief de la Tarea 74 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/prueba72` (23e70b7). Trae las Tareas 69 a 72.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-74`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/auditar.rs` nuevo y sus tests, `src/cli.rs` y `src/lib.rs` (solo el subcomando), `docs/interfaces.md`. En `src/oracle/mod.rs`, solo declarar el módulo y exponer `juez_diseno_de_produccion` como `pub(crate)`.
- **No toques:** `src/oracle/juez.rs`, `src/oracle/validez.rs`, `src/oracle/calibra/`, `src/corre/`, `src/route/`, `src/proveedores/` y `$ROOT_HOME/validez/`. `oraculo evaluar` no cambia, y `global` tampoco.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una prueba con podman y otro builder corren mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

El juez lee poco y solo reporta hallazgos con requisito afectado. La validez encontró 1 de 8 defectos reales. El usuario pidió una pasada de búsqueda que no puntúa y no toca al juez (P73 c). La medición usa defectos que vos no vas a ver. No busques defectos conocidos, y no ajustes el prompt.

Leé en el plan, antes de empezar:

- "Desempates técnicos 43 (Opus, 2026-11-06)", parte de la pasada. No abras archivos de defectos fuera del fixture.

## Tarea

La regla que manda: **la pasada propone candidatos, y un oráculo los cuenta.** Un LLM nunca decide si un defecto se encontró. Hacé un commit por punto. Cada punto empieza con su test en rojo. Los tests usan un modelo falso, y nunca una API.

1. **Subcomando.** `r00t oraculo auditar --repo <dir> --salida <archivo.json> --tope-usd <n> --gitleaks <reporte.json> [--incluir <ruta>]... [--defectos <toml>]`. `--tope-usd` y `--gitleaks` son obligatorios. `--incluir` repetido limita los archivos: sin él, entra todo `src/`. Rojo: hoy el subcomando no existe. Sin tope, o sin reporte de gitleaks, da error de uso. Un reporte ilegible no envía nada.
2. **Archivos.** Solo `.rs`. El código desde el primer `#[cfg(test)]` no entra. Cada línea va con su número real. Las líneas que cita el reporte, leído con `parsear_gitleaks`, se reemplazan como en el juez. La pasada no corre gitleaks: el orquestador le da el reporte. Los archivos se agrupan en llamadas de `MAX_ENTRADA_JUEZ_BYTES` o menos, en orden de ruta. Rojo: un archivo con tests solo manda la parte de producción.
3. **Prompt fijo.** Usá este texto, sin cambiarlo. Su hash va en la salida:

   > Sos un revisor senior. Leé este código y reportá defectos reales: conducta incorrecta, seguridad, fiabilidad o uso de recursos. Cada defecto trae archivo, línea, la cita textual de esa línea y una frase que dice qué falla. No reportes estilo. Si no hallás defectos, respondé una lista vacía. Respondé SOLO un objeto JSON: {"defectos":[{"archivo":"...","linea":N,"cita":"...","detalle":"..."}]}

4. **Modelo y tope.** Usa la elección de Route para `juzga`, con razonamiento, y `juez_diseno_de_produccion`. Antes de cada llamada, compara `gasto_api_desde_en` desde el inicio más el peor caso con el tope. Si no entra, para. La salida queda con `incompleta = true` y los archivos sin leer. Rojo: con un tope que entra una sola llamada, la segunda no se envía.
5. **Gate de cita.** Un candidato pasa solo si `evidencia::cita_en_archivo` confirma su cita. Los demás van a `descartados`, con su motivo. No hace falta requisito afectado. Rojo: un candidato con cita falsa no entra en `hallazgos`.
6. **Medición determinista.** Con `--defectos`, cada defecto cuenta como encontrado si un hallazgo cae en el mismo archivo a 3 líneas o menos. Lee la misma forma de `defectos.toml`, y usa solo los defectos cuyo `repo` termina en el nombre de `--repo`. La salida trae `encontrados`, `casos`, y el detalle de cada defecto. Rojo: con un fixture de 2 defectos y un hallazgo en uno, da 1 de 2.
7. **Salida.** Trae `hash_prompt`, `modelo`, `archivos`, `llamadas`, `costo_usd`, `hallazgos`, `descartados` e `incompleta`. La escritura es atómica. No toca `calibracion.json` ni `validez.json`.

**Guarda.** `oraculo evaluar` con los mismos datos da el mismo veredicto, porque la pasada no entra ahí.

No corras la pasada real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-74.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
