# Cómo entra la propuesta de áreas al mecanismo
Type: grilling
Status: claimed
Blocked by:
Map: ../map.md

## Question

¿Dónde corre la propuesta de áreas que decidió el ticket 01 de `sdd-fase-2`, y cómo convive con la regla de cero LLM del ticket 03?

Decisión recibida de la sesión de SDD fase 2 (2026-10-23, `brainstorms/2026-10-23-sdd-fase-2-ticket-01.md` Q15, rama `claude/gifted-bartik-c4a833`). El usuario confirmó este orden:

1. El mecanismo propone un área por carpeta, sin LLM.
2. Un LLM lee el código y sugiere ajustes sobre esa propuesta (unir o partir carpetas).
3. El usuario aprueba la propuesta final antes de que entre a `sdlc.toml`.

La propuesta por carpeta funciona sola si el LLM falla. El formato es una tabla `[areas]` en `sdlc.toml` con `nombre = [globs]`. El nombre admite `[a-z0-9_-]+` y es también la carpeta `docs/specs/<area>/`. Los globs se comparan con `_calza()`, igual que el techo `files`. Un chequeo sobre `git ls-files` lista los archivos de cada área y nombra los que caen en dos áreas.

Choque con decisiones de este mapa:

- El ticket 03 exige que `doctor` y `--upgrade` nunca llamen a un LLM (punto 4 del test).
- El ticket 01 dice que el mecanismo ofrece áreas pero nunca las escribe solo. La aprobación del paso 3 es compatible con esa regla.

Opciones a decidir:

- A. Comando aparte, por ejemplo `python -m sdlc areas <repo>`. `doctor` y `--upgrade` siguen sin LLM, y `doctor` solo avisa que el repo no declara áreas.
- B. Paso opcional de `--upgrade` con una bandera, por ejemplo `--areas`. El punto 4 del ticket 03 vale solo sin la bandera.
- C. El paso 1 (sin LLM) entra a `--upgrade` como propuesta en el reporte; el paso 2 (LLM) queda en un comando aparte.

Preguntas derivadas:

- ¿La tabla `[areas]` se inserta antes del primer encabezado de tabla, como las claves del ticket 03, o al final del archivo?
- ¿El chequeo de archivos en dos áreas es desvío o aviso de `doctor`?
- ¿Las specs planas de Adepor (ticket 06) siembran áreas por tema en vez de por carpeta?

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_propuesta_de_areas`: en `tmp_path`, un repo con tres carpetas de código recibe una propuesta de tres áreas sin llamadas al LLM. Con el `llm` simulado que falla, la propuesta por carpeta sale igual. Sin aprobación, `sdlc.toml` queda byte a byte igual. Con aprobación, `tomllib` lee `[areas]` y cada nombre cumple `[a-z0-9_-]+`. `doctor` y `--upgrade` siguen con cero llamadas al LLM.
