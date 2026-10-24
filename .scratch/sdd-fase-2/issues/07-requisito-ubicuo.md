# Qué es un requisito ubicuo
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md

## Question

¿Cómo sabe el SDLC qué requisitos `R<n>` reciben un test de propiedades?

En EARS, un requisito ubicuo vale siempre y no tiene disparador: "El sistema debe ...". El curso (nivel 5, ejercicio 5) pide un test de propiedades por requisito ubicuo. La plantilla actual (`sdlc/templates/spec-template.md`) no usa EARS. Un requisito es una línea `- R1: <regla con su orden de evaluacion>`.

Elegir qué requisitos son ubicuos es juicio sobre texto, no un chequeo. Si lo decide un LLM, la decisión es no determinista, pero no es un veredicto sobre código checkeable.

Opciones a decidir:

- A. Plantilla EARS. Los requisitos siguen los patrones EARS. Un regex detecta el ubicuo por ausencia de "Cuando", "Mientras", "Si" y "Donde".
- B. Marcador explícito. El writer marca `- R3 [prop]: ...`. Un regex lo lee. El clarify de la etapa 2b puede agregar o quitar marcas.
- C. Sección propia. La spec gana `## Propiedades`, con una línea por invariante y el `R<n>` que la origina.
- D. Todos los requisitos son candidatos. El autor del test decide cuáles admiten propiedad y deja constancia en la tabla de trazabilidad.

Preguntas derivadas:

- ¿Un requisito con disparador ("Cuando X, el sistema debe Y") también admite propiedad si X es generable?
- ¿Cuántos requisitos ubicuos tiene una spec típica del SDLC? Las specs archivadas desde `e72394c` sirven de muestra.

## Test de aceptación propuesto

`tests/test_sdlc_propiedades.py::test_detecta_requisitos_ubicuos`: una spec de juguete con cuatro requisitos, dos ubicuos y dos con disparador. La función de detección devuelve exactamente los dos ubicuos según la regla elegida. Una spec sin requisitos ubicuos devuelve la lista vacía y el SDLC sigue el camino de hoy.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-07.md`.

- Un requisito con test de propiedades lleva una marca explícita pegada al ID: `R3 [prop]: <regla>`. Un regex la lee.
- El criterio de la marca es una regla que vale para un conjunto de entradas generable, con disparador o sin él. El criterio queda escrito en `spec-template.md`.
- El writer marca en la etapa 2. La etapa 2b aplica el criterio al revisar.
- La marca vale con área y sin área. Una spec sin marcas sigue igual que hoy.
- EARS no se adopta para elegir requisitos: la detección por ausencia de disparador es frágil y no hay medición a favor. EARS queda como mejora posible de la plantilla, a medir aparte.

### Test de aceptación aprobado

`tests/test_sdlc_propiedades.py::test_detecta_marcas` usa una spec con cuatro requisitos, dos con marca:

1. La detección devuelve exactamente los dos IDs marcados.
2. Un texto que menciona `[prop]` dentro de otra regla no marca ese requisito.
3. Una marca mal escrita, como `[Prop]` o `[props]`, no cuenta como marca.
4. Una spec sin marcas devuelve una lista vacía, y la corrida sigue igual que hoy.
5. Un `MODIFIED` que quita la marca deja el requisito sin marca en la spec fusionada.
6. La detección funciona igual en la spec de un área y en `docs/sdlc/spec.md`.
7. `gate_traza_spec` sigue encontrando R3 cuando R3 lleva la marca.
8. La plantilla `spec-template.md` incluye el criterio de la marca.
9. Un `ADDED` con marca deja el requisito marcado en la spec fusionada.
10. Un `MODIFIED` que solo agrega o quita la marca pasa el gate del ticket 04.
11. La spec base que escribe la etapa 0 también puede llevar marcas.

## Comments

- 2026-10-23, enmienda desde el ticket 08 (decisión del usuario): los tests de propiedades nacen en la etapa 1 y la parada única los aprueba. Las marcas `[prop]` salen de ahí: un ID con test de propiedades lleva marca. El writer de la etapa 2 copia esas marcas y un gate exige que coincidan. La etapa 2b ya no agrega ni quita marcas por su cuenta; el criterio de la plantilla guía a Claude en la etapa 1.
