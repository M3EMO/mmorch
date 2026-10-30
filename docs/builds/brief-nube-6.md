# Brief de la build en la nube 6

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Commit de partida: la punta de `claude/root-e7813e`. Esa punta trae la mezcla de la nube 5 (6ee7c1d) y este brief.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 a 5 ya entraron a la base. Leé el informe de la nube 5 antes de empezar. Está en `docs/builds/`, con un nombre que termina en `build-nube-5.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- La tarea la construye un subagente Haiku, o uno por parte de tarea.
- La sesión revisa cada entrega antes del commit. Una corrección empieza con un test en rojo.
- La sesión no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno de la nube

La sesión no tiene podman, ni `ROOT_HOME` real, ni Colab, ni red real, ni claves de API, ni la credencial de Claude.

Primeros pasos, en este orden:

1. Corré `git config core.hooksPath .githooks`.
2. Exportá `ROOT_SIN_PODMAN=1`. Con esa variable, los tests que necesitan podman se saltean y lo avisan en stderr.
3. Corré `tools/test-seguro.sh` y anotá el resultado de base: verdes, fallas e ignorados.

Si la suite falla en la base por algo que no es el entorno, detenete y escribilo en el informe.

## Tarea

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 34 | "Tarea 34: Requisitos cruzados con el contrato (decisión 50)" |

Leé también, antes de empezar:

- "Validate real de las Tareas 7 y 9c (2026-10-30)".
- "Desempates técnicos 14 (Opus, 2026-10-30): el contrato como evidencia (decisión 50)".
- La Tarea 9c, con su regla de niveles por requisito.

Notas de la tarea:

- **Por qué.** Hoy Root cruza los `[[requisito]]` solo con el junit de la suite oculta. Seis requisitos de memora y brasa no tienen tests ocultos y quedan sin veredicto, aunque el contrato sí los prueba. El usuario eligió que el contrato también cuente como evidencia.
- **Formato de id.** Un id con el prefijo `contrato:` apunta a la suite de contrato. Un id sin prefijo sigue apuntando a la suite oculta. Los `tarea.toml` de hoy no cambian.
- **Nivel.** La regla de niveles de la Tarea 9c no cambia. Solo cambia dónde se busca cada id.
- **Confianza.** Un requisito con evidencia solo del contrato figura en `Informe.confianza` con la marca "evidencia visible al builder".
- **Regla nueva.** Un test del contrato que falla de forma estable fuera de todo requisito da `NoListo`, con la regla `test_contrato_fuera_de_requisitos`. La regla `test_oculto_fuera_de_requisitos` no cambia.
- **Campo nuevo.** `Veredicto` suma `contrato_fallidos`, con `#[serde(default)]`. Regenerá `docs/interfaces.md`.
- **MCP.** Los ids del contrato no se redactan en `root_evaluar`: el builder ya los ve.

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

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-6.md` y commitealo. Usá las mismas secciones que el informe de la nube 5:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
