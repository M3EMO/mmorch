# 05 — Captura de outcome híbrida

Type: grilling
Status: resolved
Blocked by: 04

## Question

Mecánica del veredicto: cómo se detecta la reacción explícita ("dale"/"no")
sin comando nuevo obligatorio, cuánto vale N para que ignorar = rechazo blando,
dónde persiste el estado propuesta→pendiente/aceptada/rechazada, y cómo se
mapea cada outcome a `record_outcome` para que el bandit deje de estar starved
(qué es el brazo: ¿fuente de la idea? ¿tipo de propuesta? ¿proyecto?).

## Answer

Grilling 2026-08-12.

- **Detección: ambas vías** — (1) la tarjeta inyectada instruye a la sesión a
  llamar `mmorch_record_outcome` con el id al recibir "dale"/"no"; (2) el hook
  SessionEnd (session_ingest, ya existente) barre el transcript por patrón como
  red de seguridad. Dedup por id de propuesta.
- **Brazo del bandit: FUENTE** — `propuesta:nota` vs `propuesta:roadmap-<lente>`.
  Aprende qué tipo de idea sirve; pocos brazos → converge con poco volumen
  (elegido contra proyecto y fuente@proyecto, que repetirían el starving).
- **Estado**: en `adjudications.json` mismo — cada propuesta con
  `id, status: pendiente|aceptada|rechazada|expirada, shown_count`.
- **N = 5**: mostrada 5 veces en sesión sin reacción → rechazo blando.
  Expiración a 14 días aplica igual a lo nunca mostrado.
- **Rewards**: dale = 1.0 · no explícito = 0.125 · rechazo blando
  (ignorada/expirada) = 0.2.
