# Unidad de la spec viva: feature o área
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md

## Question

¿Sobre qué unidad se escribe el delta: la spec de una feature archivada o una spec por área del código?

Hoy `_archivar_spec()` guarda `docs/specs/<slug>/spec.md`, con el slug de la tarea. Una feature nueva sobre un área vieja no sabe qué spec es su base. OpenSpec guarda una spec por capacidad (`openspec/specs/<dominio>/`) y los cambios aparte (`changes/`).

Opciones a decidir:

- A. Por feature. El delta nombra la feature base (`base: docs/specs/<slug>/spec.md`). Dos features sobre la misma área dejan dos specs que pueden contradecirse.
- B. Por área. Una spec viva por área (`docs/specs/<area>/spec.md`) y cada feature deja su delta en `docs/specs/<area>/changes/<slug>.md`. Hay que definir qué es un área (carpeta, módulo, lista en `sdlc.toml`).
- C. Mixto. Se mantiene la carpeta por feature y un índice `docs/specs/index.md` mapea archivo del repo a spec base.

Preguntas derivadas:

- ¿Quién elige el área de una tarea: el llamador (`build_feature(..., area=...)`), `sdlc.toml` o una regla sobre los archivos del plan?
- ¿Las specs ya archivadas por `orchestration-3s7` migran a la unidad nueva o quedan como están?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_spec_base_de_una_tarea`: con dos features archivadas en un `tmp_path`, una tercera tarea sobre la misma área resuelve su spec base a una ruta única y determinista. Con la opción A o C, la ruta sale del llamador o del índice. Con la opción B, la ruta es `docs/specs/<area>/spec.md`. Una tarea sin spec base devuelve `None` (caso del ticket 03).

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-01.md`.

- La spec viva es **por área** (opción B). Cada área tiene `docs/specs/<area>/spec.md`.
- Cada feature deja su delta en `docs/specs/<area>/changes/<slug>.md`.
- Cada feature deja su plan en `docs/specs/<area>/changes/<slug>.plan.md`. Con área no se crea `docs/specs/<slug>/`.
- Un área es una entrada de `sdlc.toml` con nombre y globs. Los globs se comparan con `_calza()`, como el techo del plan.
- El área de una tarea sale de los `files` del llamador. No sale del plan, porque la etapa 2 escribe el delta antes que la etapa 3.
- Sin área (sin `files`, o ningún archivo calza), la corrida sigue el camino de hoy: spec completa en `docs/specs/<slug>/`.
- La adopción de áreas es voluntaria por repo. Un repo sin áreas no cambia.
- Dos áreas o más cortan la corrida y proponen dividir la tarea, como `gate_tamano` con `docs/sdlc/division.md`.
- Un archivo que calza con dos áreas cuenta las dos y corta por la misma regla.
- Un área más archivos sin área: vale el área y los archivos sueltos se ignoran.
- No hay specs archivadas para migrar: ningún `docs/specs/*/spec.md` existe en `/home/mpera` (búsqueda del 2026-10-23).
- Un área declarada sin `spec.md` todavía queda para el ticket 03.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_spec_base_de_una_tarea` arma un `sdlc.toml` con dos áreas en un `tmp_path`:

1. `files` dentro de un área devuelven `docs/specs/<area>/spec.md`.
2. Una tarea sin `files` devuelve `None` y sigue el camino de hoy.
3. `files` que no calzan con ningún glob devuelven `None`.
4. `files` en dos áreas cortan la corrida y escriben `docs/sdlc/division.md`.
5. El corte del caso 4 ocurre antes de cualquier llamada al LLM (`llm` simulado, cero llamadas).
6. Un archivo que calza con dos áreas corta igual que el caso 4.
7. Un área más un archivo suelto devuelve el área.
8. Otro orden de los `files` da el mismo resultado.

`tests/test_sdlc_delta.py::test_archivo_con_area`: con área, la etapa 6 deja `changes/<slug>.md` y `changes/<slug>.plan.md` y no crea `docs/specs/<slug>/`. Un `sdlc.toml` sin áreas mantiene verde `tests/test_sdlc_sdd.py::test_dos_features_dejan_dos_specs`.
