# Formato del delta y su gate
Type: grilling
Status: open
Blocked by: 01, 02
Map: ../map.md

## Question

¿Qué forma tiene el delta y qué chequea su gate determinista?

OpenSpec usa `## ADDED Requirements`, `## MODIFIED Requirements` y `## REMOVED Requirements`. Un `MODIFIED` repite el requisito entero, no solo la diferencia. La doc de OpenSpec no nombra `RENAMED`.

Un gate determinista posible:

- Cada ID de `MODIFIED` y `REMOVED` existe en la spec base.
- Ningún ID de `ADDED` existe en la spec base.
- Cada ID de `ADDED` y `MODIFIED` tiene un test que lo nombra (como `gate_traza_spec`).
- Un ID aparece en una sola sección.

Opciones a decidir:

- A. El delta reemplaza a `spec.md` en la etapa 2 cuando hay spec base. La etapa 2 escribe `docs/sdlc/delta.md` y el plan lo lee.
- B. La etapa 2 escribe la spec completa como hoy y el delta sale por diff determinista entre la base y la spec nueva. El LLM no escribe el delta.
- C. La etapa 2 escribe los dos y un gate exige que el diff de ambos coincida.

Preguntas derivadas:

- ¿`REMOVED` lleva un motivo obligatorio, como en OpenSpec?
- ¿Entra `RENAMED`, o un renombre es `REMOVED` más `ADDED`?
- ¿La revisión de la etapa 2b (clarify) revisa el delta, la spec fusionada o los dos?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_gate_delta`: una spec base con R1 y R2 y cinco deltas. El delta válido (`MODIFIED R1`, `ADDED R3`) pasa. Cuatro deltas fallan, cada uno con su motivo en el mensaje del gate: `MODIFIED R9` inexistente, `ADDED R1` repetido, `R2` en dos secciones y `ADDED R3` sin test. Con la opción B, el test agrega un caso: el diff de dos specs da exactamente el delta esperado.
