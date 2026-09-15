# Lo subjetivo baja a sintetizado
Type: grilling
Status: closed (2026-09-15)
Blocked by: 02
Map: ../map.md

## Question

Un gate de juicio (revisión de spec, '¿es el producto correcto?') no tiene oráculo. Diseñar cómo produce ejemplos etiquetados: cada revisión humana deja 1 ejemplo (bueno o malo) con motivo; con 3 buenos + 3 malos se sintetiza un checker que los separa; se promueve contra ellos; el gate baja de juicio a sintetizado. Decidir: formato del ejemplo, dónde se guardan, quién promueve, cuándo un checker sintetizado sobre subjetivo se considera confiable (no hay verdad computada). El hook ETS es el precedente: 'hablame simple' → 3 reglas medibles.

## Resolucion (2026-09-15, HITL; research en vault `ticket-08-sdlc-como-lo-subjetivo-baja-a-checker-5-opciones-m`)

- D1 (usuario acepta): el productor de ejemplos es la aprobacion del test de aceptacion (etapa 1, ticket 12). Cada
  veredicto guarda el test completo, la etiqueta (aprobado/rechazado) y un motivo de una linea.
- D2 (usuario elige A+E+D de la research): el resume desde la etapa 2 EXIGE etiqueta y motivo (cerrado por default: sin
  veredicto no se reanuda) y los guarda en `logs/sdlc/veredictos.jsonl` (append-only). El mutation score del test de
  aceptacion sobre el codigo final entra como gate ejecutable de la etapa 5 (D: rojo en HEAD + mata mutantes = "el test
  mide la tarea" sin opinion); umbral por repo en sdlc.toml (`mutation_min`), solo observa hasta que haya numero medido.
  El checker sintetizado (A) se entrena con los veredictos y corre EN SOMBRA hasta kappa >= 0.6 contra el humano; aun
  promovido, el humano decide los casos borde (E). El motivo es la rubrica de A (C es gratis).
- Fuera del ticket: quien etiqueta ademas del humano (Hermes) es el ticket 14.
