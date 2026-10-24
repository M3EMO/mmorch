# Spec base de un área sin spec
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-03.md`.

- El SDLC suma una etapa "0-caracterizar" para los archivos de la tarea que la spec del área todavía no nombra en su sección Contrato.
- Claude escribe los tests de caracterización por la CLI, como en la etapa 1. Cada corrida deja su propio archivo `tests/test_sdlc_<area>_base_<slug>.py`.
- Claude solo puede escribir ese archivo. Tocar código u otros tests rechaza la etapa (`gate_alcance`).
- Gates deterministas: los tests pasan en HEAD y matan al menos un mutante del archivo caracterizado. El puntaje queda en el run-log.
- Un repo puede fijar `caracterizacion_min` en `sdlc.toml`, con el esquema de `mutation_min`.
- Las etapas 0 y 1 corren seguidas. La corrida se detiene una sola vez con `approve_accept` para aprobar los dos tests juntos.
- Tras la aprobación, el writer por API escribe la spec base desde los tests aprobados. Después, la etapa 2 escribe el delta.
- Los requisitos base toman el menor número libre del área (ticket 02) y citan sus tests con `ruta::test`.
- Los tests aprobados entran a `SNAP`. Un delta que cambia un comportamiento caracterizado queda para el ticket 05.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_caracterizacion` usa un repo de juguete con un área declarada y un archivo sin cubrir:

1. Un test que pasa en HEAD y mata un mutante pasa el gate.
2. Un test que falla en HEAD se rechaza y el gate nombra el test.
3. Un test vacuo, como `assert True`, se rechaza porque no mata ningún mutante.
4. Con `caracterizacion_min = 0.5`, un puntaje de 0.3 se rechaza.
5. Un archivo que la sección Contrato ya nombra no se vuelve a caracterizar.
6. La corrida se detiene una sola vez, con los tests de caracterización y de aceptación juntos.
7. Con `approve_accept = false`, la corrida sigue sin parada.
8. Tras la aprobación, la spec base cita cada test de caracterización con `ruta::test`.
9. Un área sin archivos nuevos y una corrida sin área no corren la etapa 0.
10. Claude que toca código u otro test en la etapa 0 rechaza la etapa.
11. Los mutantes salen solo del archivo caracterizado, no del área entera.
12. Tras la aprobación, un coder que edita un test de caracterización falla `gate_baseline`.
13. Los requisitos de la spec base toman el menor número libre del área.
