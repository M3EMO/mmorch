# Unidad de la spec viva: feature o área
Type: grilling
Status: open
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
