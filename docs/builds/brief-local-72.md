# Brief de la Tarea 72 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `e665a5f`, la punta de `claude/root-e7813e`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-72`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/validez.rs`, sus tests, `tests/` de validez y `docs/interfaces.md`.
- **No toques:** el resto de `src/oracle/`, `src/corre/`, `src/route/`, `src/proveedores/`, `src/config.rs` y `$ROOT_HOME/validez/defectos.toml`. Las métricas `exactitud_orden` y `recall` no cambian de valor.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Los builders 70 y 71 corren mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La validez reprobó, con orden 5 de 6 y recall 1 de 8, y `validez.json` no dice qué par falló ni qué defecto se encontró. Una repetición cuesta cerca de 1 USD. Sin detalle, esa plata no explica nada.

Leé en el plan, antes de empezar:

- "Diagnóstico de la validez (Opus, sin gastar API)".
- "Desempates técnicos 42 (Opus, 2026-11-06)".

## Tarea

La regla que manda: **medir igual, y dejar a la vista por qué.** Hacé un commit por punto. Cada punto empieza con su test en rojo. Los campos nuevos llevan `#[serde(default)]`. Usá el evaluador falso de los tests de validez, con veredictos de la forma real.

1. **Veredictos guardados.** Cada repo evaluado escribe `<carpeta de casos>/veredictos/<nombre del clon>.json`, con el veredicto entero, de forma atómica. Rojo: hoy no queda nada.
2. **Detalle de pares.** `Validez` suma `pares`. Cada uno trae `tarea`, `mejor`, `peor`, `acierto`, y de cada lado su decisión, `global` y completitud. Trae también `criterio`: `decision`, `global`, `completitud` o `sin_orden`. Rojo: hoy un par fallido no se identifica.
3. **Detalle de defectos.** `Validez` suma `defectos`. Cada uno trae `tarea`, `repo`, `archivo`, `linea`, `encontrado`, y el hallazgo más cercano del mismo archivo con su línea, su fuente y su distancia. Sin hallazgo en el archivo, ese dato queda en `null`. Rojo: hoy un defecto encontrado no se identifica.
4. **Evidencia cercana.** Cada defecto suma `en_evidencia`: si la evidencia de un ítem cita ese archivo a 3 líneas o menos. Leé la forma real de `NivelItem.evidencia` antes de escribirlo. No cuenta para `encontrado`. Rojo: un veredicto con esa evidencia y sin hallazgo debe dar `encontrado = false` y `en_evidencia = true`.
5. **Test del defecto.** `DefectoReal` acepta un campo opcional `test`. Con él, el detalle suma `test_rojo`: si el test está en `ocultos_fallidos`, `inestables` o sus listas del contrato. No cuenta para `recall`. Rojo: hoy un defecto con `test` da error por `deny_unknown_fields`.

**Guarda final.** Con los mismos casos, `exactitud_orden.valor` y `recall.valor` no cambian.

**Pregunta para el informe.** ¿Los hallazgos que el juez descarta llegan al `Veredicto`? Si no llegan, escribilo, sin cambiarlo.

No corras la validez real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-72.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
