# Brief de la build en la nube 4

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Ese commit viene después de 0a26553.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1, 2 y 3 ya entraron a la base. Leé el informe de la nube 3 antes de empezar: `docs/builds/2026-10-30-build-nube-3.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- Cada tarea la construye un subagente Haiku, uno por tarea o por parte de tarea.
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

Cada tarea está en el plan, con su `Action`, su `Validate` y su parte "Dónde". Construí solo la parte de nube de cada una.

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 31 | "Tarea 31: Seguridad en todo lenguaje y sondas de configs conectadas" |
| 2 | 32 | "Tarea 32: Orden de las reglas del informe (espera P58)" |

Leé también, antes de empezar:

- "Mezcla de la nube 3 (2026-10-30)".
- "Hallazgo de Seguridad (2026-10-30)".
- "Desempates técnicos 12".
- La decisión 48, en la tabla "Decisiones del usuario" y en la sección "Decisión 48 (2026-10-29): P56 c y P57 a".

Notas por tarea:

- **31** tiene muchos desempates. Partila en commits por grupo: sondas de Seguridad y de configs, informe y calibración, descarga, validez, runner, y tests de `tools/test-seguro.sh`.
- **31**: `gitleaks` y las sondas de configs corren para toda tarea, con cualquier `lenguaje`. En la nube, el ejecutor es falso.
- **31**: un ítem sin herramienta para el lenguaje va a `faltantes` con el motivo "sin sonda para <lenguaje>". La dimensión conserva el veredicto de sus ítems medidos.
- **31**: no cambies el orden de las reglas de `decidir`. Ese cambio es de la Tarea 32.
- **32** espera la respuesta del usuario a la pregunta P58. Hacela solo si el orquestador te manda un mensaje con esa respuesta. Si la respuesta es a), seguí la sección de la Tarea 32. Si es b), no cambies `decidir` y anotalo en el informe. Si no llega ningún mensaje, no hagas la 32 y anotalo en el informe.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por tarea, o por parte de tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios: los precios por defecto ya están en el código.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`. Los temporales van bajo `TMPDIR`, que fija `tools/test-seguro.sh`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si una tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-4.md` y commitealo. Usá las mismas secciones que el informe de la nube 3:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
