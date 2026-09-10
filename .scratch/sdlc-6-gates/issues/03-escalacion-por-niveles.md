# Escalacion por niveles
Type: grilling
Status: resolved
Blocked by:
Map: ../map.md
Capture: `brainstorms/2026-09-10-sdlc-6-gates.md` (Q11–Q13)
Industria: vault loops-de-parche + `research/industria-por-ticket.md`

## Question

Diseñar niveles de escalación cuando un gate falla.

## Decisions

- Niveles: (1) fix loop 3 vueltas; (2) reasoner 2 veces, una instrucción por archivo; (3) Claude en sesión; (4) humano.
- Disparo: 3 fallos de (1) → (2). 2 fallos de (2) o USD agotado → (3). Zona roja → (4) directo.
- Aviso a persona: al pasar a Claude, o zona roja. No en cada rojo del nivel 1.
- Deja: (1) diff acotado + log; (2) instrucción + parche + candidato en `supervision.md`; (3) corrección + candidato; (4) decisión humana.
- Claude puede promover solo `sintetizado` o `determinista` con evidencia ya corrida. No promueve `juicio`.
- Job anota el nivel (`gate_failed` / `escalated`).
- ¿El nivel 2 elimina la mayoría de las escaladas a Claude? Se mide en el ticket 04.
