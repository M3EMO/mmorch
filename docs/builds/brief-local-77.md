# Brief de la Tarea 77 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-76` (c88040b).
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-77`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/`, `src/observe/transcript.rs`, `src/oracle/auditar.rs`, `Fila` en `src/tipos.rs` (solo campos nuevos), `build.rs` nuevo, `docs/interfaces.md` y sus tests.
- **No toques:** `PROMPT_AUDITORIA`, `src/oracle/juez.rs`, `src/oracle/validez.rs`, `src/oracle/perfiles/`, `src/oracle/calibra/`, `src/route/`, `src/proveedores/` y `$ROOT_HOME/`. `oraculo evaluar` no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una suite con podman y la validez corren mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La corrida real terminó bien, pero su fila miente en cuatro puntos. Dice `invalida` por un `ps` del builder. Sus minutos y turnos ignoran las vueltas cortadas. No dice qué binario la produjo. Además, `descartados` de `auditar` puede copiar un secreto.

Leé en el plan: "Vuelta del 2026-11-06 (7)", parte de los tres defectos.

## Tarea

La regla que manda: **la fila dice lo que pasó.** Hacé un commit por punto. Cada punto empieza con su test en rojo, con datos de la forma real. Ningún test usa una API ni imprime un token.

1. **Sin ruta del host en los argumentos.** `args_claude` deja de agregar `Read`, `Edit`, `Grep` y `Glob` con rutas de `ROOT_HOME`. Queda `Bash(*oraculos*)`. Rojo: hoy los argumentos contienen la ruta de `home`. Actualizá los tests que pedían esas reglas, y decilo en el informe.
2. **Ruta literal solo en la entrada.** En `buscar_en`, la búsqueda literal de `huellas.rutas` corre solo con `es_entrada`. Armá el caso con una línea `user` con `tool_result` real, con la salida de `ps` que trae la ruta. Rojo: hoy ese caso da `OraculoLeido`. Guardas:
   - `cat <ruta de oraculos>` en la entrada sigue dando evento;
   - una huella o un nombre del oráculo en la salida siguen dando evento.
3. **Minutos y turnos de un corte.** Un corte también registra un `Sesion`, con `latency_s` igual a la duración real del lanzamiento, `extra.num_turns` igual a los `message.id` distintos de `assistant`, y `extra.cortada = true`. Su costo sigue en la `Llamada`: el `Sesion` lleva 0. Rojo: con 2 cortes y un `result`, hoy `minutos` y `turnos` cuentan solo el último.
4. **`commit_binario`.** `build.rs` guarda el commit al compilar, con el sufijo `-sucio` si el árbol tiene cambios, y `desconocido` sin git. `Fila` suma `commit_binario`, con `#[serde(default)]`. `commit_root` no cambia. Rojo: hoy la fila no lo trae. Revisá que el worktree con `.git` en archivo funcione, y que `cargo` recompile al cambiar de commit.
5. **Cita no confirmada.** En `auditar`, un candidato con "cita no confirmada en el archivo" va a `descartados` sin `cita` ni `detalle`, con `cita_hash`. Rojo: hoy copia la cita. La medición no cambia.

**Guardas.** `hash_prompt` no cambia. Una fila vieja se lee igual.

No corras la corrida real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-77.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
