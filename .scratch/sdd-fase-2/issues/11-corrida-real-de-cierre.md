# Corrida real de cierre
Type: grilling
Status: open
Blocked by: 03, 05, 06, 09, 10
Map: ../map.md

## Question

¿Qué corrida real decide que las dos capacidades funcionan?

El SDLC es un builder (memoria `sdlc-es-builder`): un cambio se juzga con tests, gates y corridas reales, sin A/B del bench. Este ticket fija la corrida, no la ejecuta.

Opciones a decidir:

- Repo: orchestration (dogfooding), uno de los repos adoptados (Estudio, Portfolio, ChatBot) o F2.
- Feature: un cambio real que modifique un comportamiento ya cubierto por tests, para que el delta tenga un `MODIFIED`.
- Señal de las propiedades: un defecto real del coder que solo atrapa un test de propiedades, o un defecto plantado. Un defecto plantado por el mismo autor del test prueba poco (`builder-e2e`, nota sobre la prueba fuerte del driver).

Preguntas derivadas:

- ¿Cuánto cupo y cuántos USD acepta el usuario para la corrida?
- ¿Cuántas paradas humanas acepta: aprobación de la base, del delta y de los tests de propiedades?
- ¿Qué cuenta como fracaso: el delta falla su gate, la suite marca regresión falsa, o la propiedad sale vacua?

## Test de aceptación propuesto

El run-log de una corrida real (`logs/` del SDLC) muestra cuatro hechos. Primero, la feature parte de una spec base y deja un delta que pasa su gate. Segundo, la spec fusionada queda en el PR. Tercero, la suite total no marca como regresión los tests de los IDs del delta. Cuarto, al menos un test de propiedades mata un mutante propio, sin un LLM en el veredicto.

## Comments

- 2026-10-23, desde el grilling del ticket 01: el repo de la corrida necesita áreas declaradas en `sdlc.toml`. El esfuerzo aparte `orchestration-4u6` (bd) decide cómo se actualizan los repos propios a las convenciones. Si ese esfuerzo no terminó, el área se declara a mano en el repo de la corrida.
- 2026-10-23, desde el ticket 01 (aprobado por el usuario): ningún test unitario prueba que `bd` corre en el host y no en el contenedor. La corrida real verifica que el issue `sdlc-fuera-de-area:<area>` aparece en el bd correcto.
