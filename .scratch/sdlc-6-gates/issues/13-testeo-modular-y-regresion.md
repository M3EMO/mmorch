# Testeo modular + regresión por unidad
Type: grilling
Status: resolved
Blocked by: 02
Map: ../map.md
Capture: `brainstorms/2026-09-10-sdlc-6-gates.md` (Q8–Q10)
Industria: `research/industria-por-ticket.md`

## Question

¿El pipeline tiene testeo modular y total, para ver si una integración corrompe algo?

## Decisions

- Ninguna unidad aterriza sin tests de esa unidad. Sintetizados desde la spec. `clase: sintetizado`, `alcance: unidad`.
- Hard gate: `mutation_score`. Si no mata mutantes: no es gate (advisory o se descarta).
- Cobertura no es hard gate hasta tener número medido.
- Suite total corre después de cada unidad. Reporta: unidad X rompió test Y. Complementa la aceptación final. No la reemplaza.
- `test-compile` antes del test, fail-closed.
- Umbral de mutación: campo en `GATE-N.md` del repo. No copiar 80/85 de PIT/Stryker. El número lo mide el ticket 04.
- Tres capas: unidad (mutantes) + total/integración (corrupción) + aceptación (producto).
