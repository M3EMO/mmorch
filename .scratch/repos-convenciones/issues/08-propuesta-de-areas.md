# Cómo entra la propuesta de áreas al mecanismo
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-24). Registro completo: `brainstorms/2026-10-24-repos-convenciones-ticket-08.md`.

- La propuesta corre en un comando aparte, `python -m sdlc areas <repo>`. `doctor` y `--upgrade` siguen sin LLM.
- `areas <repo>` escribe `docs/sdlc/areas-propuesta.toml`. El usuario puede editar ese archivo antes de aprobarlo.
- `areas <repo> --aprobar` valida la propuesta con las reglas de `doctor` y la pasa a `sdlc.toml`.
- `--aprobar` sigue las reglas del ticket 07: rama default, aborta si `sdlc.toml` está sucio, commitea solo `sdlc.toml` y pushea siempre. Después borra la propuesta.
- Un agente pide OK antes de `--aprobar`, porque pushea. `areas <repo>` sin `--aprobar` no commitea ni pushea.
- El paso 2 usa `DEFAULT_GENERATOR` (`mmorch/config.py`) con `mmorch.providers.call`. El modelo devuelve JSON con operaciones `unir` o `partir` y un motivo.
- Un validador determinista aplica las operaciones válidas. Una respuesta inválida o una falla deja la propuesta por carpeta sin cambios.
- `[areas]` y `[spec_de_area]` van como tablas al final de `sdlc.toml`.
- Un archivo que calza con dos áreas es desvío de `doctor` y rechazo de `--aprobar`.
- La propuesta lista las specs planas sin área, y el LLM recibe sus títulos como contexto. El usuario asigna en el archivo de propuesta.
- `doctor` avisa "sin áreas" solo cuando el repo tiene specs planas en `docs/specs/`.
- El área `raiz` lista cada archivo suelto de la raíz por su nombre exacto. Techo y áreas comparten la semántica de `_calza()`, donde `*` cruza carpetas (ticket 01 de `sdd-fase-2`).
- Un repo que ya declara `[areas]` no recibe propuesta. Cambiar áreas declaradas es una edición a mano de `sdlc.toml`.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_propuesta_de_areas` arma repos en `tmp_path` con un `llm` simulado y verifica diecinueve puntos:

1. Un repo con tres carpetas de código recibe una propuesta de tres áreas en `docs/sdlc/areas-propuesta.toml`.
2. Con el `llm` simulado que falla, la propuesta por carpeta sale igual.
3. Una operación `unir` válida del `llm` une dos áreas, y la propuesta anota su motivo.
4. Una operación que nombra una carpeta inexistente se descarta, y una respuesta que no es JSON también.
5. Sin `--aprobar`, `sdlc.toml` queda byte a byte igual y no hay commit.
6. Con `--aprobar`, `tomllib` lee `[areas]` al final del archivo, y cada nombre cumple `[a-z0-9_-]+`.
7. Tras `--aprobar`, la propuesta desaparece, el commit contiene solo `sdlc.toml` y el remoto bare recibe el push.
8. `--aprobar` rechaza una propuesta con un archivo en dos áreas, sin escribir nada.
9. `--aprobar` aborta en una rama que no es la default o con `sdlc.toml` sucio.
10. Un área que el usuario renombró a mano en la propuesta entra con ese nombre.
11. La propuesta lista las specs planas sin área, y el prompt del `llm` contiene sus títulos.
12. `doctor` avisa "sin áreas" solo si `docs/specs/` tiene specs planas, y da desvío por un archivo en dos áreas.
13. `doctor` y `--upgrade` siguen con cero llamadas al `llm`.
14. Un repo que ya declara `[areas]` hace que `areas` aborte con un mensaje, sin escribir una propuesta.
15. Una propuesta existente nunca se pisa, así que `areas` aborta si el archivo ya existe.
16. El paso 1 toma carpetas con código trackeado y salta `tests/`, `docs/` y las carpetas con punto, como `init()`.
17. El área `raiz` lista cada archivo suelto por nombre exacto, y ningún archivo de la propuesta calza con dos áreas.
18. Un nombre de carpeta inválido, como `Motor_Cuotas`, pasa a minúsculas, y un choque de nombres aborta la propuesta.
19. La llamada al `llm` queda registrada en `logs/metrics.jsonl`.
