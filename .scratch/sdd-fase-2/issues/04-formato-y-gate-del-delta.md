# Formato del delta y su gate
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-04.md`.

- El writer por API escribe solo el delta, con secciones `ADDED`, `MODIFIED` y `REMOVED`. Un `MODIFIED` repite el requisito entero, como OpenSpec.
- El código fusiona el delta con la spec base (ticket 06). Después exige que el diff entre base y spec fusionada nombre exactamente los IDs del delta.
- Un delta que olvida un requisito cambiado queda atrapado por la suite: el test de ese ID falla (regla del ticket 05).
- Cada `REMOVED` lleva un motivo obligatorio. El gate solo chequea que exista. El motivo pasa a `docs/specs/<area>/removidos/README.md`.
- No hay `RENAMED`: cambiar el texto es `MODIFIED`, y el ID nunca cambia (ticket 02).
- La etapa 2b lee la spec fusionada como contexto y edita solo el delta (`gate_alcance`). `## Clarificaciones` queda en el delta.
- La etapa 1 recibe la spec del área y los próximos números libres, calculados por código. La etapa 0 asigna primero los IDs base.
- Cada `test_R<n>` de la etapa 1 corresponde a un `ADDED` o `MODIFIED` del delta.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_gate_delta` usa una spec base con R1, R2 y R4. Cada fallo nombra su motivo en el mensaje del gate.

1. Un delta válido con `MODIFIED R1`, `ADDED R3` y `REMOVED R2` con motivo pasa el gate.
2. Un `MODIFIED R9` falla, porque R9 no existe en la base.
3. Un `ADDED R5` falla, porque el menor número libre es R3.
4. Un `R1` en dos secciones falla.
5. Un `ADDED R3` sin test `ruta::test` falla.
6. Un `REMOVED R2` sin motivo falla.
7. El diff entre la base y la spec fusionada nombra exactamente R1, R2 y R3.
8. Una fusión que cambia R4 por error falla el chequeo del caso 7.
9. La etapa 2b que edita la spec fusionada o la base rechaza la etapa con `gate_alcance`.
10. `gate_clarificaciones` encuentra `## Clarificaciones` en el delta.
11. Un encabezado desconocido, como `## MODIFED`, falla.
12. El gate vuelve a correr tras la etapa 2b, y un delta roto por la revisión falla.
13. Una corrida sin área no escribe delta ni corre este gate.
14. Cada `test_R<n>` de la etapa 1 corresponde a un `ADDED` o `MODIFIED` del delta.
15. Un `MODIFIED R1` con el mismo texto que la base falla con el motivo "MODIFIED sin cambio".
16. La etapa 1 que nombra un test con un número fuera de la lista ofrecida falla en su propio gate.
