# Qué es un requisito ubicuo
Type: grilling
Status: open
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
