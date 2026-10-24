# Propiedades sin duplicar gate_mutacion
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-24). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-09.md`.

- `gate_mutacion` no cambia su número de corridas. Por cada mutante, el run-log guarda qué test falló.
- Una propiedad que no mata ningún mutante queda marcada como vacua. Un test de ejemplo nunca queda marcado.
- La marca de vacua bloquea solo cuando el repo fija `mutation_min`, porque ahí corren todos los mutantes. Sin `mutation_min`, la marca queda anotada.
- Dentro de los mutantes, pytest corre sin `-x`. Con `-x`, una propiedad no corría si el test de ejemplo fallaba primero.
- Dentro de los mutantes, Hypothesis corre un perfil con 20 ejemplos. La aceptación normal usa el defecto. El mecanismo del perfil pertenece al ticket 10, y el número se mide en el ticket 11.
- Las relaciones metamórficas de `builder-e2e` viven en el driver de producto y no cuentan como propiedades.

### Test de aceptación aprobado

`tests/test_sdlc_propiedades.py::test_propiedad_vacua_detectada` usa un módulo de juguete, un test de ejemplo y dos tests de propiedades:

1. Una propiedad mata un mutante que el test de ejemplo no mata, y el run-log lo registra.
2. La otra propiedad no mata ningún mutante, y el run-log la nombra como vacua.
3. El número de corridas de la aceptación por mutante es igual al de hoy.
4. Sin `mutation_min`, la propiedad vacua queda anotada y el gate pasa.
5. Con `mutation_min`, la propiedad vacua bloquea el gate, y el mensaje la nombra.
6. Dentro de los mutantes, Hypothesis usa el perfil de 20 ejemplos.
7. La aceptación normal usa el número de ejemplos por defecto.
8. Un test de ejemplo nunca queda marcado como vacuo.
9. El cálculo de `mutation_score` no cambia respecto de hoy.
10. Un mutante que mata primero el test de ejemplo igual registra el resultado de la propiedad.
11. Con `accept_cmd` de otro lenguaje, el run-log avisa que no hay atribución por test, y el gate queda igual que hoy.
