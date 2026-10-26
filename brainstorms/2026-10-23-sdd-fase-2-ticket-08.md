# SDD fase 2, ticket 08 (etapa y autor de los tests de propiedades): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir en qué etapa nace el test de propiedades y quién lo escribe. Ticket: `.scratch/sdd-fase-2/issues/08-etapa-de-las-propiedades.md`.

Insumos ya decididos: marca `R3 [prop]: <regla>` pegada al ID, criterio de conjunto generable, vale con y sin área (ticket 07); la etapa 1 recibe la spec del área y nombra tests con el ID final (ticket 04); una sola parada aprueba caracterización y aceptación (ticket 03); un test cubre un solo ID y cada ID tiene su archivo (ticket 05).

## Summary / key decisions

- Los tests de propiedades nacen en la etapa 1, junto con los de ejemplo; la parada única los aprueba (Q1).
- Las marcas `[prop]` salen de la etapa 1: un ID con test de propiedades lleva marca; el writer de la etapa 2 las copia y un gate exige que coincidan (enmienda al ticket 07).
- Las propiedades pueden pasar en HEAD; cada ID `ADDED`/`MODIFIED` necesita al menos un test rojo (Q2).
- El coder por API no ve el código de las propiedades, solo la salida del fallo; Claude por la CLI sí puede leerlo (límite aceptado, Q3 y Q4).
- Test de aceptación aprobado con trece casos (Q5). Ticket 08 resuelto.

## Q&A log

### Q1 — Etapa del test de propiedades
- Asked: ¿etapa 1 con aprobación en la parada única (A), etapa 2c que lee las marcas (B) o 2c sin aprobación (C)?
- Captured: el usuario elige **A**. Un test de propiedades es un oráculo; el humano aprueba todos los oráculos en una sola parada. B obligaba a una segunda parada o a un oráculo sin aprobar; C dejaba a un LLM fijar un oráculo sin revisión.
- Captured: enmienda al ticket 07: las marcas salen de la etapa 1; el writer las copia; un gate exige igualdad entre IDs marcados e IDs con test de propiedades. La etapa 2b ya no agrega marcas por su cuenta.
- Hecho (sin pregunta): con un archivo por ID (ticket 05), el test de propiedades de R3 vive en el mismo archivo que el de ejemplo, `tests/sdlc/<area>/test_R3_<slug>.<ext>`; sin área, en el archivo de aceptación de hoy.
- Flags: si cada test de propiedades debe fallar en HEAD (Q2).

### Q2 — Rojo en HEAD para propiedades
- Asked: ¿cada test de propiedades rojo en HEAD (A), regla de hoy con el archivo entero rojo (B) o propiedades exentas del rojo con al menos un test rojo por cada ID `ADDED`/`MODIFIED` (C)?
- Captured: el usuario elige **C**. Una propiedad que ya vale hoy protege lo que no debe cambiar. Cada ID `ADDED` o `MODIFIED` necesita al menos un test rojo en HEAD. La vacuidad de las propiedades la mide el ticket 09 con mutantes. A rechazaba propiedades correctas.
- Flags: si el coder de la etapa 4 ve los tests de propiedades (Q3).

### Q3 — El coder ve los tests de propiedades
- Asked: ¿visibles como los de ejemplo (A), código oculto y solo la salida del fallo (B) o fuera del worktree con oráculo externo (C)?
- Captured: el usuario elige **B**.
- Hallazgo de Claude: B se cumple solo para el coder por API, que recibe el texto por `_tests_text()`. Claude por la CLI (escalera nivel 3 y coder `claude:<modelo>`, como Haiku) corre dentro del worktree con herramientas y puede leer el archivo. Cumplir B con Claude exige sacar el archivo del worktree, que es la opción C (Q4).

### Q4 — Cómo se cumple B con Claude por la CLI
- Asked: ¿aceptar el límite y ocultar solo al coder por API (A), sacar los archivos de propiedades del worktree en las etapas 4 y 5 (B) o volver a tests visibles (C)?
- Captured: el usuario elige **A**. `_tests_text()` excluye el código de los tests de propiedades del prompt del coder por API; el coder sigue viendo la salida del fallo. Claude por la CLI (escalera nivel 3, coder `claude:<modelo>`) puede leer el archivo: límite aceptado y documentado.
- Hecho (sin pregunta): un `MODIFIED` de un ID marcado mueve el archivo viejo, con su test de propiedades, a `removidos/` (ticket 05); el archivo nuevo de la etapa 1 lleva propiedad solo si el bloque nuevo lleva marca.
- Flags: test de aceptación (Q5).

### Q5 — Test de aceptación
- Asked: ¿aprobás el test con once casos? Después: ¿falta algo?
- Captured: el usuario aprueba trece casos: los once más una propiedad de la etapa 0 que debe pasar en HEAD, y ningún prompt por API (armado, elección de archivos, arreglo de regresiones) que incluya el código de las propiedades.
- Flags: determinismo de Hypothesis y otros lenguajes -> ticket 10. Ticket 08 resuelto.

## Open flags (pending input)
