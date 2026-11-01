# Brief de la build en la nube 9

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Commit de partida: lo pone el orquestador. Es la punta de `claude/root-e7813e` que trae este archivo, después de la mezcla de la nube 8 (92d8b18).
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 a 8 ya entraron a la base. Leé el informe de la nube 8 antes de empezar: `docs/builds/2026-10-31-build-nube-8.md`.

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
| 1 | 40 | "Tarea 40: Desempates de la nube 8, tope 2 de inestables y redondeo" y "Desempates técnicos 18" |

Leé también, antes de empezar:

- "Mezcla de la nube 8 y Validate de 7 y 9c, intento 4 (2026-10-31)".
- La Tarea 9c, con su regla de niveles por requisito.

Notas de la tarea:

- **Tope 2 de inestables.** Hoy `requisitos::verde()` cuenta solo `Pasa`, así que un test inestable cuenta como "no verde" y deja su requisito en nivel 1. La Tarea 9c pide otra regla: el nivel se calcula con el test inestable como verde, y después queda con tope 2.
- **Pregunta P59.** El usuario puede elegir una regla por tasa de pase. Si el orquestador te manda la respuesta de P59, seguí esa regla. Si la respuesta es b), un test inestable cuya cota inferior de Wilson al 90 % es menor que 0,5 cuenta como falla, y uno con una cota de al menos 0,5 pone tope 2. Usá la misma cota de Wilson que `track`. Si no llega ningún mensaje, aplicá la regla de la Tarea 9c y anotalo en el informe.
- **Redondeo.** Un motivo que compara contra un umbral muestra tres decimales. Si los dos números siguen iguales con tres decimales, muestra cuatro.
- **Colab.** El aviso de una omisión de `[[entorno]]` llega a `Veredicto.avisos`. El paquete separa `tests/oculto/` y `tests/contrato/`, cada una con su conftest, y no deja ningún conftest en `tests/`.
- **Historial.** La calibración no escribe historial. `evaluar` borra, por `src/borrado.rs`, los historiales de versiones sin uso en 30 días.
- **Texto.** Un test inestable se describe con "pasa X de N" una sola vez.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por parte de la tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`. Los temporales van bajo `TMPDIR`, que fija `tools/test-seguro.sh`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si la tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-9.md` y commitealo. Usá las mismas secciones que el informe de la nube 8:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
