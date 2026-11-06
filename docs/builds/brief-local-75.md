# Brief de la Tarea 75 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/prueba74`. Trae las Tareas 69 a 74.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-75`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/auditar.rs` y sus tests, `src/cli.rs` y `src/lib.rs` (solo este subcomando) y `docs/interfaces.md`.
- **No toques:** `PROMPT_AUDITORIA`, `src/oracle/juez.rs`, `src/oracle/validez.rs`, `src/oracle/calibra/`, `src/corre/`, `src/route/`, `src/proveedores/`, `$ROOT_HOME/` y `~/.cache/root-wt/`. No abras archivos de defectos fuera del fixture.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La corrida real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Con un tope insuficiente, `auditar` hizo 0 llamadas y su salida informó "0 de 10", sin leer nada. La Línea Buscador va a comparar variantes con este subcomando, y un conteo falso la invalida. Tampoco puede medir candidatos de otro modelo.

Leé en el plan, antes de empezar: "Desempates técnicos 46 (Opus, 2026-11-06)" y la parte de la recomendación 1. No necesitás nada más.

## Tarea

La regla que manda: **sin lectura no hay medición.** Hacé un commit por punto. Cada punto empieza con su test en rojo. Los tests usan un modelo falso, y nunca una API.

1. **Tope insuficiente.** Si el peor caso de la primera llamada no entra en `--tope-usd`, el subcomando sale con 2, dice la cifra que necesita y no escribe medición. La fórmula de la reserva no cambia. Rojo: hoy sale con 0 y escribe "0 de N".
2. **Sin lectura, sin conteo.** Un defecto en un archivo que no se leyó queda con `medido = false`. `medicion` suma `casos_leidos`, y `encontrados` cuenta solo sobre esos. Sin ningún archivo leído, `medicion` queda `null`. Rojo: hoy un defecto no leído cuenta como no encontrado.
3. **Salida incompleta.** Con `incompleta = true`, el subcomando sale con 3, y escribe igual su JSON. Rojo: hoy sale con 0.
4. **`--por-archivo`.** Cada archivo va en su propia llamada, con el mismo prompt. Un archivo que no cabe se parte como hoy. Rojo: dos archivos chicos hoy van en una llamada.
5. **`--solo-medir <candidatos.json>`.** Toma candidatos con la forma `{"defectos":[...]}` de otro modelo. No llama a ningún modelo ni pide tope ni clave. Aplica el gate de cita y la medición. Rojo: hoy no existe.
6. **Clase y origen.** Cada defecto acepta `clase` y `origen` (`real` o `sembrado`), opcionales. `medicion` suma el conteo por clase y por origen. Rojo: un fixture con 2 orígenes hoy da un solo conteo.
7. **Densidad.** La salida suma `lineas_leidas` y `candidatos_por_1000_lineas`. Rojo: hoy no están.

**Guarda.** `hash_prompt` no cambia, y `oraculo evaluar` da el mismo veredicto.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-75.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
