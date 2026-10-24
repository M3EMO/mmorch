# Etapa y autor de los tests de propiedades
Type: grilling
Status: resolved
Blocked by: 07
Map: ../map.md

## Question

¿En qué etapa nace el test de propiedades y quién lo escribe?

Problema de orden: la etapa 1 escribe la aceptación ANTES de la spec, y la etapa 2 escribe la spec desde esos tests fijos. Una propiedad "desde la spec" no tiene spec a la que mirar en la etapa 1.

Opciones a decidir:

- A. Etapa 1 ampliada. Claude escribe los tests de ejemplo y los de propiedades juntos, desde la tarea. El humano aprueba los dos en `awaiting_approval`. Los requisitos ubicuos salen de la tarea, no de la spec.
- B. Etapa nueva "2c-propiedades" después del clarify. El autor lee la spec revisada y escribe `test_R<n>_prop_*` solo para los ubicuos. Los tests entran a `SNAP` como la aceptación.
- C. Etapa 2c sin aprobación humana. Igual que B, pero los gates deterministas del ticket 09 reemplazan la aprobación.

Preguntas derivadas:

- ¿Autor: Claude (cupo), el writer por API (cero cupo) o el verificador `deepseek-reasoner`?
- ¿El coder de la etapa 4 ve los tests de propiedades, o quedan fuera del worktree como la verdad del ticket 03 de `builder-e2e`?
- ¿Un test de propiedades nuevo debe fallar en HEAD (rojo por diseño, como la etapa 1)? Una propiedad de un requisito sin cambios puede pasar ya en HEAD.
- ¿Un requisito `MODIFIED` del delta (rama 01 a 06) recibe test de propiedades nuevo, o conserva el viejo?

## Test de aceptación propuesto

`tests/test_sdlc_propiedades.py::test_etapa_de_propiedades`: con el LLM simulado (como en `tests/test_sdlc_gates.py`), una spec con un requisito ubicuo produce un archivo con `test_R<n>_prop_*` que usa `@given`. El archivo entra a la aceptación que corre `accept()`. Un test de propiedades sin `@given` o sin el ID en el nombre corta la etapa con un gate que nombra el motivo.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-08.md`.

- Claude escribe los tests de propiedades en la etapa 1, junto con los de ejemplo. La parada única los aprueba todos.
- El test de propiedades de un ID vive en el archivo de ese ID (`tests/sdlc/<area>/test_R<n>_<slug>.<ext>`). Sin área, vive en el archivo de aceptación de hoy.
- Las marcas `[prop]` salen de la etapa 1. El writer de la etapa 2 las copia, y un gate exige igualdad entre IDs marcados e IDs con test de propiedades (enmienda al ticket 07).
- Una propiedad puede pasar en HEAD. Cada ID `ADDED` o `MODIFIED` necesita al menos un test rojo en HEAD. La vacuidad la mide el ticket 09.
- Ningún prompt del coder por API incluye el código de las propiedades. El coder ve la salida del fallo.
- Límite aceptado: Claude por la CLI (escalera nivel 3 y coder `claude:<modelo>`) corre en el worktree y puede leer el archivo.
- Un `MODIFIED` de un ID marcado mueve el archivo viejo, con su propiedad, a `removidos/` (ticket 05).

### Test de aceptación aprobado

`tests/test_sdlc_propiedades.py::test_etapa_de_propiedades` usa el LLM simulado, como en `tests/test_sdlc_gates.py`:

1. La etapa 1 deja un `test_R<n>_prop_*` con `@given` en el archivo del ID.
2. Un test de propiedades sin `@given` corta la etapa 1, y el gate nombra el motivo.
3. Un test de propiedades sin el ID en el nombre también corta la etapa 1.
4. Una marca sin test de propiedades hace fallar el gate de la etapa 2.
5. Un test de propiedades sin marca también hace fallar ese gate.
6. Una propiedad verde en HEAD pasa si su ID tiene otro test rojo.
7. Un ID `ADDED` con todos sus tests verdes en HEAD corta la etapa 1.
8. La parada única lista los tests de propiedades junto con los demás.
9. El prompt del coder por API no incluye el código de las propiedades, pero sí la salida del fallo.
10. `accept()` corre los tests de propiedades como parte de la aceptación.
11. Sin área, la propiedad vive en el archivo de aceptación de hoy, con las mismas reglas.
12. Una propiedad de la etapa 0 debe pasar en HEAD, como todo test de caracterización.
13. Ningún prompt por API incluye el código de las propiedades, ni en la etapa 4 ni en la escalera.
