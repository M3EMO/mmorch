# Brief de la Tarea 65 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-64` (8c4d382). No esperes ninguna mezcla.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-65`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/mutacion.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/`, `tests/fixtures/`, `docs/interfaces.md` y sus tests.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/juez.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- El Grupo 1 de la calibración corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La sonda de mutación toma los primeros 60 mutantes del listado. En memora son 2 archivos de 16, y solo un test por socket los ejercita. Por eso `AssertBorrado` no encuentra candidato, y Pruebas no mide casi nada del repo. Esta tarea cambia una evaluación real: la muestra queda repartida por todo el listado.

Leé en el plan, antes de empezar:

- "Diagnóstico de `assert_borrado` en memora (Opus, 2026-11-05)".
- "Desempates técnicos 36 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un test con ejecutor falso devuelve lo mismo que la herramienta real.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 5.

1. **Muestra repartida.** Con más mutantes que `mutantes_max`, la muestra toma los índices `i * total / max`, para `i` de 0 a `max - 1`. Con menos, toma todos. Rojo: un listado de 3 archivos con 100 mutantes y máximo 10 hoy toca solo el primero, y debe tocar los 3. El truncado y `total_conocido` no cambian.
2. **Acotado por `-F`.** Quitá `--file` del listado y de la corrida. `con_archivo_acotado` sigue: el listado corre entero, la muestra sale igual que en la base, y después se queda solo con los nombres de ese archivo. Rojo:
   - un archivo con más mutantes que el máximo: el mutante de la muestra de la base que está lejos del inicio debe entrar en el filtro;
   - ningún comando lleva `--file`;
   - sin nombres de ese archivo en la muestra, la sonda queda sin veredicto y no corre la corrida.
3. **Motivos separados.** Un `AssertBorrado` no aplicable dice una de dos cosas: "la base no dio mutantes muertos" o "ningún mutante muerto tiene un test con asserts que lo llame". Rojo: hoy los dos casos dicen "sin candidato para inyectar".
4. **Punta a punta con listado real.** Calibrá Pruebas con `calibrar_tarea_con` y `evaluar_con` real, con un ejecutor que responde al listado y a la corrida según el filtro `-F` que recibe, como `cargo-mutants`. El fixture tiene 60 mutantes iniciales sin test por nombre, y mutantes posteriores con tests unitarios. Rojo: hoy los casos con defecto son no aplicables. Después deben aplicar y dar `mutante_vivo:`.
5. **Herramienta real.** Un test con `cargo-mutants` real, que corre solo con podman, usa un fixture de dos archivos dentro de `con_archivo_acotado`. Comprueba que el reporte trae solo mutantes de ese archivo. Vos no lo corrés: lo corre el orquestador. Es una guarda, y el informe lo dice.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-65.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
