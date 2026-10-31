# Brief de la build en la nube 7

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Commit de partida: la punta de `claude/root-e7813e`. Esa punta trae la mezcla de la nube 6 (cde8ac5) y este brief.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 a 6 ya entraron a la base. Leé el informe de la nube 6 antes de empezar: `docs/builds/2026-10-30-build-nube-6.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- Cada tarea la construye un subagente Haiku, uno por tarea.
- La sesión revisa cada entrega antes del commit. Una corrección empieza con un test en rojo.
- La sesión no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno de la nube

La sesión no tiene podman, ni `ROOT_HOME` real, ni Colab, ni red real, ni claves de API, ni la credencial de Claude.

Primeros pasos, en este orden:

1. Corré `git config core.hooksPath .githooks`.
2. Exportá `ROOT_SIN_PODMAN=1`. Con esa variable, los tests que necesitan podman se saltean y lo avisan en stderr.
3. Corré `tools/test-seguro.sh` y anotá el resultado de base: verdes, fallas e ignorados.

Si la suite falla en la base por algo que no es el entorno, detenete y escribilo en el informe.

## Tareas, en orden

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 36 | "Tarea 36: Desempates de la nube 6" y "Desempates técnicos 15" |
| 2 | 35 | "Tarea 35: Suite por worktree y un lock de podman (regla 4)" |

Leé también, antes de empezar:

- "Estado (2026-10-31): nube 5 mezclada, nube 6 en prueba".
- "Regla 4 de la laptop (2026-10-31)".
- "Desempates técnicos 14 (Opus, 2026-10-30): el contrato como evidencia (decisión 50)".

Notas por tarea:

- **36, entorno.** Una exclusión de `[[entorno]]` acepta el prefijo `contrato:`. Un id sin prefijo excluye solo el test oculto.
- **36, MCP.** `redactar_para_mcp` conserva las entradas `contrato:` de la evidencia de los requisitos. Quita solo las entradas ocultas.
- **36, evidencia visible.** Un requisito entra en `evidencia_visible` si ninguno de sus ids ocultos aporta un resultado medido: pasa, falla o inestable.
- **35, carpeta por worktree.** `tools/test-seguro.sh` arma su carpeta con un hash corto de la ruta del repo: `$HOME/.cache/root-tests/<hash>/`. Adentro van `target`, el `ROOT_HOME` de prueba y el `TMPDIR`.
- **35, lock.** Sin `ROOT_SIN_PODMAN`, el script toma un lock con `flock` sobre `$HOME/.cache/root-tests/podman.lock` y espera su turno. Con `ROOT_SIN_PODMAN=1`, no toma el lock.
- **35, prueba del lock.** Probalo con dos corridas del script en paralelo sobre un comando falso. No hace falta podman.
- **35, hook.** El cambio de carpeta obliga a una compilación completa en el primer commit. Es esperado.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`. Los temporales van bajo `TMPDIR`, que fija `tools/test-seguro.sh`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si una tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-7.md` y commitealo. Usá las mismas secciones que el informe de la nube 6:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
