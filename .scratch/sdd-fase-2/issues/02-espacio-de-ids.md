# Espacio de IDs entre features
Type: grilling
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿Cómo se nombran los requisitos para que un delta cite sin ambigüedad un `R<n>` de otra feature?

Hoy cada feature numera desde `R1`. La etapa 1 exige `def test_R<n>_...` y `gate_traza_spec` busca `\bR\d+\b`. Dos features dejan dos `R1` distintos. Un `MODIFIED R1` no dice cuál de los dos cambia. Los tests viejos `test_R1_*` de features distintas viven juntos en `tests/`.

Opciones a decidir:

- A. IDs locales a la spec base. El delta numera dentro de la spec del área (sale del ticket 01, opción B). `ADDED` sigue la numeración de la base.
- B. Prefijo de área o feature: `TOPE-R3`. Cambian el regex de `gate_traza_spec`, de `gate_traza_plan` y de la etapa 1.
- C. IDs locales, con el nombre del archivo de test como espacio de nombres (`tests/test_sdlc_<slug>.py::test_R3_*`). El delta cita `<slug>:R3`.

Preguntas derivadas:

- ¿Un ID eliminado (`REMOVED`) se puede reusar después?
- ¿Las features ya archivadas se renumeran?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_ids_sin_colision`: dos specs archivadas con `R1` cada una y un delta `MODIFIED` sobre una de ellas. El resolvedor de IDs devuelve el requisito exacto de la spec base y el test viejo que lo cubre. Un delta que cita un ID ambiguo falla con un mensaje que nombra los candidatos. `gate_traza_spec` y `gate_traza_plan` siguen verdes con los tests existentes de `tests/test_sdlc_gates.py`.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-02.md`.

- La numeración vive dentro de la spec del área. El regex `\bR\d+\b` de los gates no cambia.
- Un `ADDED` toma el menor número libre del área. El hueco de un ID eliminado dura hasta el próximo cambio.
- No se renumera nunca: renumerar renombra tests, rompe deltas viejos y referencias en commits y bd.
- La tabla de trazabilidad del área cita cada test con `ruta::test`, el formato de pytest. Los nombres `test_R<n>_*` de la etapa 1 no cambian.
- Un ID eliminado se reusa. Sus tests no se borran: se mueven a `docs/specs/<area>/removidos/`, fuera de la suite.
- `docs/specs/<area>/removidos/README.md` lista cada ID eliminado. La spec viva no lo lista, así que `gate_traza_spec` no cambia.
- Una corrida sin área mantiene los IDs por feature, como hoy.
- No hay specs archivadas para renumerar.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_ids_sin_colision` arma dos áreas en un `tmp_path`:

1. Un `ADDED` en un área con R1, R2 y R4 toma R3.
2. Un `ADDED` en un área con R1 a R3 toma R4.
3. Dos `ADDED` en el mismo delta, sobre un área con R1, R2, R4 y R5, toman R3 y R6.
4. Un `MODIFIED R1` resuelve el requisito de la spec del área y su test por `ruta::test`.
5. Una tabla del área que cita un test sin ruta falla el gate y nombra el test.
6. Una cita `ruta::test` a un test que no existe falla el gate y nombra la ruta.
7. Un `REMOVED R2` mueve su test a `docs/specs/<area>/removidos/` y suma R2 al `README.md`.
8. La suite `pytest tests` no recolecta el test movido.
9. Un R2 reusado más tarde cita su test nuevo, y el test viejo sigue en `removidos/`.
10. Un R2 eliminado dos veces conserva los dos tests movidos, sin pisar el primero.
11. Una corrida sin área mantiene los IDs por feature, y `tests/test_sdlc_gates.py` sigue verde.
