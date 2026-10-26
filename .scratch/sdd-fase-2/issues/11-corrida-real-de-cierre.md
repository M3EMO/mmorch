# Corrida real de cierre
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-24). Registro completo: `brainstorms/2026-10-24-sdd-fase-2-ticket-11.md`. El ticket fija la corrida; no la ejecuta.

- Dos corridas de cierre: orchestration con Python y F2 con TypeScript.
- orchestration: cambio en `tools/ste-lint.py` sobre cómo `sentences()` corta oraciones, más la baja de una regla vieja. Propiedad: el conteo de palabras del texto entero iguala la suma por oración.
- F2: `validarBruto()` de `src/validacion.ts` acepta el formato argentino `1.234.567,89`. `MODIFIED`: el punto pasa a separador de miles. `REMOVED`: el punto deja de ser separador decimal. Propiedades: ida y vuelta, y `validarBruto()` nunca lanza excepción.
- Evidencia de las propiedades: al menos una mata un mutante que los tests de ejemplo no matan (ticket 09).
- Tope de gasto: `usd_max = 3.0` por corrida, el de hoy. El cupo de Claude queda anotado.

### Test de aceptación aprobado

Precondiciones:

- El código de los tickets 01 a 10 y el ticket 12 está implementado, con sus tests verdes.
- orchestration declara un área para `tools/ste-lint.py` y tiene Hypothesis instalado.
- F2 declara un área para `src/validacion.ts` y tiene fast-check instalado.
- Ningún área de esos repos tiene spec todavía, así que la etapa 0 corre.

Cada corrida cumple doce condiciones en su run-log:

1. La etapa 0 deja tests de caracterización verdes en HEAD que matan mutantes.
2. La parada única muestra caracterización, aceptación, propiedades y bajas juntas.
3. El delta tiene al menos un `MODIFIED` y un `REMOVED`, y pasa su gate.
4. La spec fusionada y el delta quedan en el primer commit de la etapa 6.
5. La suite no marca como regresión los tests movidos a `removidos/`.
6. Al menos una propiedad mata un mutante que los tests de ejemplo no matan.
7. El gasto de API queda bajo 3 dólares, y el cupo de Claude queda anotado.
8. Si el plan intenta salir del área, el issue `sdlc-fuera-de-area:<area>` aparece en el bd correcto. Si no ocurre, el run-log lo anota y la condición no falla.
9. El run-log anota el tiempo de la etapa de mutantes y los ejemplos usados dentro de ellos.
10. El pre-commit del ticket 06 pasa en los dos commits de la etapa 6.
11. Correr de nuevo los gates sobre el mismo commit da el mismo veredicto.
12. El run-log anota cuánto tiempo pasó la corrida esperando la aprobación humana.

El mapa falla si alguna condición falla en alguna de las dos corridas. Cada falla vuelve como ticket nuevo a este mapa.
- 2026-10-24, desde `orchestration-4u6` (ticket 09 de `.scratch/repos-convenciones/`): las precondiciones de librería se cumplen con `python -m sdlc propiedades <repo> --aprobar` en orchestration y en F2. F2 usa `contenedor`, así que la imagen se reconstruye con el comando de `sdlc/builder.py`.
