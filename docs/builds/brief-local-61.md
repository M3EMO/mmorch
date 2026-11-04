# Brief de la Tarea 61 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de la Tarea 58**. El orquestador te dice el commit.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-61`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/`, `src/oracle/contenedor.rs`, en `src/oracle/mod.rs` solo lo necesario para que el aviso de dependencias llegue a `Confianza.avisos`, y en `src/oracle/calibra.rs` solo `Guardar`. También sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/descarga*`, `src/oracle/entrega.rs`, `src/oracle/memo.rs`, `src/proveedores/`, `src/observe/`, `src/config.rs` y el resto de `src/oracle/mod.rs` y de `src/oracle/calibra.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.


- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Los informes de las Tareas 55, 57 y 59 dejaron cinco ajustes chicos. Leé `docs/builds/2026-11-04-build-local-55.md`, `docs/builds/2026-11-04-build-local-57.md` y `docs/builds/2026-11-04-build-local-59.md`.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04 (tarde): 55, 57 y 59 mezcladas, corrida real de memora en curso".
- "Desempates técnicos 31 (Opus, 2026-11-04)".
- La sección de tu tarea en el plan.

## Tarea

Hacé un commit por punto.

1. **Alias del modelo (55-P2).** El costo de un rol cortado traduce el alias del modelo, como `haiku`, con `ruteo.api_equivalente` antes de buscar el precio. Si no hay precio, sigue el precio pesimista. Test: un rol `haiku` cortado usa el precio de `claude-haiku-5-5`.
2. **Mensaje de escritura (55-P5).** Un fallo de escritura del log dice "sesión no registrada", no "sesión no leída".
3. **Códigos de podman en el oráculo (55-P6).** `src/oracle/contenedor.rs` trata 125, 126, 127 y 137 como infraestructura. Test: un comando que sale con 137 da un error de infraestructura.
4. **Aviso de dependencias (57-P1).** El aviso "dependencias de runtime no verificadas" llega a `Confianza.avisos`.
5. **`Guardar` atómico (59-P11).** La copia del `target` va a una carpeta temporal y se renombra al terminar. Una copia que falla a mitad no deja un `target` que se reuse.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-61.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
