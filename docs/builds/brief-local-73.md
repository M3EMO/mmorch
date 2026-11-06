# Brief de la Tarea 73 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/prueba72` (23e70b7). Trae las Tareas 69 a 72.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-73`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/validez.rs`, `src/oracle/juez.rs`, `src/oracle/mod.rs` (solo pasar los descartes al veredicto), `src/tipos.rs` (solo campos nuevos), sus tests y `docs/interfaces.md`.
- **No toques:** `PROMPT_SISTEMA`, `src/oracle/calibra/`, `src/corre/`, `src/route/`, `src/proveedores/`, `src/config.rs` y `$ROOT_HOME/validez/`. El prompt del juez no cambia: la clave de los limpios sigue igual.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una prueba con podman y otro builder corren mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

El usuario decidió que un test oculto en rojo cuenta como encontrado, con la fila separada (P74). Además, un hallazgo que el juez descarta desaparece sin rastro, y por eso no se puede saber si el juez vio un defecto. La validez repetida cuesta cerca de 1 USD, y debe traer estos datos.

Leé en el plan, antes de empezar:

- "Diagnóstico de la validez (Opus, sin gastar API)".
- "Desempates técnicos 43 (Opus, 2026-11-06)".
- Los puntos 2 y 4 de "Desempates técnicos 24".

## Tarea

La regla que manda: **medir igual, y dejar a la vista por qué.** Hacé un commit por punto. Cada punto empieza con su test en rojo. Los campos nuevos llevan `#[serde(default)]`.

1. **Recall por línea y por test.** Un defecto cuenta en `recall` si tiene un hallazgo a 3 líneas o menos, o si declara `test` y `test_rojo` es verdadero. `Validez` suma `recall_linea` y `recall_test`, con la misma forma de `Metrica` y sin umbral. El umbral 0,8 sigue en `recall`. Rojo:
   - un defecto sin hallazgo y con test rojo hoy no cuenta, y debe contar en `recall` y `recall_test`, no en `recall_linea`;
   - un defecto sin `test` y sin hallazgo no cuenta;
   - un defecto con los dos cuenta una vez en `recall`, y una vez en cada fila aparte.
2. **Hallazgos descartados.** `Veredicto` suma `hallazgos_descartados`: `archivo`, `linea`, `item` y `motivo`. Entra todo hallazgo que el juez rechazó por "sin requisito afectado" o por el gate de citas. `redactar_para_mcp` y `ocultar_tests` los quitan. No cuentan para `encuentra` ni para ningún puntaje. Armá el caso con `juez::juzgar` real y un modelo falso. Rojo: hoy un hallazgo sin impacto no deja rastro.
3. **Descarte cercano.** El detalle de cada defecto suma `descartado_cerca`: si un hallazgo descartado cae a 3 líneas o menos. Rojo: un veredicto con ese descarte debe dar `encontrado = false` y `descartado_cerca = true`.
4. **Nivel `2.0`.** Un `2.0` vale como 2. Un `2.5` sigue siendo inválido. Rojo: hoy un `2.0` va al reintento.
5. **Motivo del tope.** Un reintento frenado por el tope deja el motivo "reintento no enviado por el tope de API". Rojo: hoy ese motivo no existe.

**Guardas.** La huella de `juez::huella_del_prompt` no cambia. Un veredicto viejo sin los campos nuevos se lee igual.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-73.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
