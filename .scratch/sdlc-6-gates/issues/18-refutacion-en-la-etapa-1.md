# Refutación del test en la etapa 1 del pipeline
Type: task
Status: closed (2026-09-17, premisa refutada)
Blocked by: 17
Map: ../map.md

## Question

Conectar la skill aprobada en el ticket 17 a la etapa 1 de `mmorch/sdlc.py` (ticket 14, D9/D11): antes de
`awaiting_approval` (y antes del veredicto en modo grill), la skill propone hasta 3 tests; cada uno compila, falla en
HEAD y nombra R<n>, o se descarta y solo se cuenta; el usuario ve las propuestas junto al test y decide cada una; las
aceptadas entran al test con commit en la rama; cada decision queda en `logs/sdlc/veredictos.jsonl` como ejemplo de
refutacion (propuesta + etiqueta + motivo). Una reescritura de la skill queda solo si el banco completo no empeora (D7).

Hecho cuando: una corrida real por el server muestra propuestas filtradas antes del veredicto, con tests que prueben el
filtro (no compila, pasa en HEAD, sin R<n>) y el registro idempotente de las decisiones.

## Cierre (2026-09-17)

No se construye: el ticket 17 midio la skill y NINGUNA version llega al criterio D6 (25 corridas, 0 aciertos). Conectar
a la etapa 1 un refutador que no atrapa defectos solo agregaria ruido y costo antes del veredicto humano. La idea vuelve
a la niebla del mapa con su numero medido; el banco del ticket 16 permite reintentar cuando haya una hipotesis nueva
(por ejemplo un refutador que vea una implementacion, no solo el test).
