# Spec base de un área sin spec
Type: grilling
Status: open
Blocked by: 01
Map: ../map.md

## Question

¿Cómo nace la spec del comportamiento actual cuando el área no tiene spec archivada?

El curso (nivel 6.1) pide dos artefactos antes del delta: una spec del comportamiento actual y tests de caracterización que lo fijan. Casi todo el código de los repos del usuario se escribió antes de `orchestration-3s7`, así que este caso es el común.

Un oráculo posible ya existe en piezas: un test de caracterización debe pasar en HEAD, y `mmorch.checkers._mutants` mide si mata mutantes del área.

Opciones a decidir:

- A. Etapa nueva "0-caracterizar". Un LLM redacta la spec actual y los tests `test_R<n>_*` de caracterización. Gates: los tests pasan en HEAD y matan al menos un mutante del área. Aprobación humana como `approve_accept`.
- B. Paso manual. El usuario (o una sesión de Claude) deja la spec base y sus tests antes de correr el SDLC. El SDLC exige la base y corta sin ella.
- C. Sin spec base. La primera feature sobre el área escribe una spec completa como hoy. Los deltas empiezan desde la segunda feature.

Preguntas derivadas:

- ¿Quién escribe en la opción A: Claude (cupo) o el writer por API (cero cupo)?
- ¿Cuál es el piso de mutantes muertos para aceptar la caracterización? Sin número medido, el gate puede solo observar, igual que `gate_mutacion` sin `mutation_min`.
- ¿La caracterización cubre el área entera o solo los archivos que la tarea nombra?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_caracterizacion_verde_en_head`: en un repo de juguete con una función sin spec, la etapa de caracterización (o el chequeo de la base, en la opción B) acepta un test que pasa en HEAD y rechaza dos casos. El primer rechazo es un test que falla en HEAD. El segundo es un test vacuo (`assert True`) que no mata ningún mutante. Con la opción C, el test verifica que una tarea sin base sigue el camino de hoy sin cambios.
