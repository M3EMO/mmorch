# Propiedades sin duplicar gate_mutacion
Type: grilling
Status: open
Blocked by: 08
Map: ../map.md

## Question

¿Qué aporta un test de propiedades que `gate_mutacion` no mida ya, y cómo se mide ese aporte sin un LLM?

Lo que hace `gate_mutacion` hoy: muta las líneas cambiadas por la feature y corre la aceptación entera por mutante. Sin `mutation_min`, prueba una muestra de 5 y observa. El costo por mutante es la aceptación completa (`orchestration-cbr`: 10 a 30 min en Java y TS).

Dos riesgos opuestos:

- Duplicación. Si los tests de propiedades entran a la aceptación, `gate_mutacion` ya los cuenta. Un gate aparte que vuelve a mutar paga el costo dos veces.
- Vacuidad. Un test de propiedades puede pasar siempre (`assert True`, una estrategia que nunca genera el caso). La aceptación de ejemplo puede tapar esa vacuidad en el puntaje global.

Opciones a decidir:

- A. Sin gate nuevo. Los tests de propiedades entran a `TASK.accept_files` y `gate_mutacion` los mide junto con los de ejemplo. El run-log guarda el puntaje igual que hoy.
- B. Aporte marginal. Sobre los mismos mutantes de `gate_mutacion`, el run-log guarda cuáles mata cada grupo (ejemplo, propiedades). Un test de propiedades que no mata ningún mutante propio queda marcado vacuo. El gate reusa las corridas, no muta dos veces.
- C. Gate de vacuidad solo. Cada test de propiedades debe matar al menos un mutante del código de su requisito. No se compara contra los tests de ejemplo.

Preguntas derivadas:

- ¿La marca de vacuo bloquea o observa? Sin número medido, la regla del repo es observar.
- ¿Cuántos ejemplos corre Hypothesis dentro del gate de mutación (`max_examples`)? Cada mutante multiplica ese número.
- ¿Las relaciones metamórficas de `builder-e2e` (ticket 05) cuentan como propiedades?

## Test de aceptación propuesto

`tests/test_sdlc_propiedades.py::test_propiedad_vacua_detectada`: un módulo de juguete, un test de ejemplo y dos tests de propiedades. Uno de los tests de propiedades mata un mutante que el de ejemplo no mata. El otro es vacuo. Tras `gate_mutacion`, el run-log nombra el test vacuo y cuenta los mutantes por grupo según la opción elegida. El número de corridas de la aceptación por mutante no crece respecto de hoy.
