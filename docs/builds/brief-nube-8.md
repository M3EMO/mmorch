# Brief de la build en la nube 8

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Commit de partida: lo pone el orquestador. Es la punta de `claude/root-e7813e` que trae este archivo, después de la mezcla de la nube 7 (acda9e6).
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 a 7 ya entraron a la base. Leé el informe de la nube 7 antes de empezar: `docs/builds/2026-10-31-build-nube-7.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- La tarea la construye un subagente Haiku, o uno por parte de tarea.
- La sesión revisa cada entrega antes del commit. Una corrección empieza con un test en rojo.
- La sesión no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno de la nube

La sesión no tiene podman, ni `ROOT_HOME` real, ni Colab, ni red real, ni claves de API, ni la credencial de Claude.

Primeros pasos, en este orden:

1. Corré `git config core.hooksPath .githooks`.
2. Exportá `ROOT_SIN_PODMAN=1`. Con esa variable, los tests que necesitan podman se saltean y lo avisan en stderr. El script tampoco toma el lock de podman.
3. Corré `tools/test-seguro.sh` y anotá el resultado de base: verdes, fallas e ignorados.

Si la suite falla en la base por algo que no es el entorno, detenete y escribilo en el informe.

## Tarea

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 37 | "Tarea 37: Desempates de la nube 7" y "Desempates técnicos 16" |

Leé también, antes de empezar:

- "Estado (2026-10-31): nube 6 y nube 7 mezcladas".
- "Desempates técnicos 14" y "Desempates técnicos 15", con el formato de id `contrato:`.

Notas de la tarea:

- **Formato de id.** Un id con el prefijo `contrato:` apunta a la suite de contrato. Un id sin prefijo apunta a la suite oculta. Las seis partes de la tarea siguen esa regla.
- **Colab.** El paquete de Colab corre también las entradas `contrato:` de `[[entorno]]`. Un id sin prefijo se busca solo en `oculto/`. Si ese id existe solo en `contrato/`, no entra al paquete y deja un aviso.
- **`[[oraculo_dudoso]]`.** Acepta el prefijo `contrato:`, igual que `[[entorno]]`. La suite del contrato saca esos tests.
- **MCP.** `Veredicto.pendientes` conserva las entradas `contrato:` en `root_evaluar`. Quita solo las ocultas.
- **Evidencia visible.** Un requisito entra en `evidencia_visible` solo si al menos un test `contrato:` aporta un resultado medido y ningún test oculto lo aporta.
- **`ficha revisar`.** Sin los flags del juez, la salida sigue en 3. Sumá `--solo-deterministas`: con ese flag, la salida es 0 o 1 según las reglas deterministas, y el texto dice "juez: no pedido". Regenerá `docs/interfaces.md` si cambia la ayuda de la CLI.
- **Test inestable.** `test_seguro_dos_rutas_dan_dos_carpetas` falló una vez con `ETXTBSY` ("Text file busy"). Un test escribe un ejecutable falso y lo lanza mientras otro hilo todavía tiene ese archivo abierto para escritura. Cada ejecutable de `tests/test_seguro.rs` se escribe en un archivo temporal, se cierra y recién después se renombra a su nombre final. Probalo con 20 corridas seguidas de `cargo test --test test_seguro`.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por parte de la tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`. Los temporales van bajo `TMPDIR`, que fija `tools/test-seguro.sh`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si la tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-8.md` y commitealo. Usá las mismas secciones que el informe de la nube 7:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
