# Espacio de IDs entre features
Type: grilling
Status: open
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
