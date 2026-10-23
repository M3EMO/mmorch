# Etapa y autor de los tests de propiedades
Type: grilling
Status: open
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
