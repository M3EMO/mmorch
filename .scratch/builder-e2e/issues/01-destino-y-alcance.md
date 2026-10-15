# Destino y alcance del builder
Type: grilling
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Qué significa "armar desde cero" para el usuario, y cuándo el builder cumplió?

- Qué tipos de producto arma primero, y cuál es el primer producto de prueba.
- Qué parte del veredicto acepta que sea juicio y no cómputo.
- Cuánta intervención humana acepta por producto, y en qué puntos.
- Qué presupuesto acepta por producto, en cupo y en dólares.

Es HITL: lo responde el usuario en una sesión `/grill-me`. El agente nunca responde su propio grilling.

## Answer

Resuelto (2026-10-15) por grilling con el usuario. Captura completa: `brainstorms/2026-10-14-builder-e2e-destino.md`.

- "Desde cero" = un entregable nuevo en un repo privado nuevo; el builder es general (productos, servicios, flujos, funcionalidades).
- Primer entregable: F2, calculadora de sueldo neto. Termina en el preview local verificado; publicar es otro agente.
- Juicio: la capa computable bloquea; el juicio usa rúbricas con anclas y comparaciones de a pares, siempre en sombra hasta medirlo. Valor × esfuerzo × riesgo ordena módulos.
- Humano: walkthrough como Artifact con ok por sección, y revisión final del preview; menos de 1 h por entregable.
- Presupuesto: APIs US$5 por entregable. Claude: Opus descompone y arma el walkthrough, Sonnet planea, Claude con navegador maneja el preview. Coder: Haiku 5.5 o DeepSeek v4 pro por sorteo por módulo.
