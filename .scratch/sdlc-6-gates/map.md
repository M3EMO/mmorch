# Mapa wayfinder — SDLC de 6 etapas con gates
Label: wayfinder:map · Creado: 2026-09-10 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Todos los proyectos del usuario construyen sus features con el mismo pipeline de 6 etapas (intent → spec → plan → build → test → PR) y gates automáticos: deterministas o checkers sintetizados del `synth_store`. Claude entra solo por escalación y deja siempre una corrección y un chequeo nuevo. mmorch aporta el engine (reemplaza a `/project`); cada repo lleva el contrato (GATE-N.md, plantillas intent/spec/plan, comando de aceptación) como convención scaffoldeable. Es una arquitectura de proyectos, no una feature de Lotus. Se mide por feature: aceptación, USD, minutos, intervenciones, contra 1 etapa.

## Notes

- Decisiones ya tomadas por el usuario (2026-09-10, grilling de charteo): (1) el pipeline reemplaza a project-build, no convive; (2) la escalación es en niveles, automática hasta agotar presupuesto y después avisa — el diseño es el ticket 03; (3) la validación usa features reales del mapa de QueTePario (tickets 13-18) Y tareas sintéticas del bench; (4) la revisión de la spec antes del build es un gate de juicio FIJO, siempre.
- Evidencia base (leer antes de resolver tickets): `docs/ab-sdlc-2026-09-10/` (A/B/C: engine vs 6 etapas automático vs híbrido; lo mueve el ticket 10), README "Measured", vault "Checkers sintetizados por tipo". Gists: B verde $1.13/22 min/0 intervenciones; C verde $0.16/9 min/2 intervenciones; A rojo tras 4 fixes del engine (commits 58ce334, ea8b775). Las 2 intervenciones de C se reemplazan por gates deterministas (test-compile, regex sobre plan.md).
- Invariantes: especificidad de la cadena = su peor gate; gate LLM sin número medido no entra; sintetizador = deepseek-reasoner, coder = deepseek-v4-pro (medidos); cross-family no decorrelaciona por sí solo (co-fallo cadenas_sin_ab), razonamiento y método sí.
- Skills: `/grill-me` y `/domain-modeling` para grilling; `/research` AFK; `/prototype` para el driver v3. Research al vault global (`mmorch_vault_write`, tag `mmorch`).
- Reglas: resolver tickets es HITL; refutar por default; ambiguo = no concluyente; GOAL.md no se toca; avisar antes de superar ~$15 en un paso.
- Re-scope del usuario (2026-09-10, tras el charteo): el destino NO es integrar a Lotus; es una arquitectura de proyectos para que TODOS sus repos corran así, robustos desde el arranque. Lotus queda como observabilidad opcional. Tickets 11 y 12 salen de esto.
- Este mapa incluye UN prototipo (ticket 04) porque la decisión "¿llega a 0 intervenciones?" solo se responde con un artefacto corriendo. El resto es planificación.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Cierre del A/B y tabla final](issues/10-cierre-del-ab-y-tabla-final.md) — A rojo a un método de verde (US$0.81, 4 fixes de engine); B verde US$1.13/22 min/0 intervenciones; C verde US$0.16/9 min/2 intervenciones reemplazables por gates. Evidencia en `docs/ab-sdlc-2026-09-10/`.
- [Inventario de gates existentes](issues/01-inventario-gates-existentes.md) — 20 gates en código, todos sin número medido propio; intent, spec y PR tienen cero gates; tres contratos de salida distintos (CheckResult / tuple / int|None) que el ticket 02 debe unificar; el engine no consume synth_store ni coverage/mutation. Detalle: [research/01-inventario-gates.md](research/01-inventario-gates.md).
- [Fix loops en otros agentes](issues/09-fix-loops-en-otros-agentes.md) — nadie reescribe archivos enteros; gate por vuelta solo lint (SWE-agent, Aider), sin revert automático; tope 3 en tres sistemas; stuck-detector y 'gate antes de aplicar' son las ideas para el ticket 03. Detalle: [research/09-fix-loops.md](research/09-fix-loops.md).

## Not yet specified

- Contenido de `GATE-N.md` para las etapas sin gate medido todavía: intent (¿qué chequeo tiene una intención?), spec (más allá del contrato cubierto), PR (diffstat, archivos permitidos, ¿review LLM advisory?). Depende del ticket 02.
- Cómo `supervision.md` alimenta el `synth_store`: formato del "chequeo nuevo" que Claude deja al escalar, y quién lo promueve. Depende de 03 y 08.
- Cuándo y cómo se retira el engine viejo (project_driver / project_integrate): flag, rama, o borrado. Depende de 05.
- Topes de costo por feature y por nivel de escalación (USD y minutos). Depende de 03 y 07.
- Validación en ChatBot: `main` ya tiene Spring y el webhook; el baseline del A/B es anterior. Cómo se elige el punto de partida de cada feature de validación. Depende de 06.
- Observabilidad opcional en Lotus: checkpoint por gate. Baja de prioridad por el re-scope; depende de 05.
- Cómo se versiona el contrato por repo cuando el engine cambia (compatibilidad GATE-N.md ↔ engine). Depende de 11.

## Out of scope

- Dashboard, WhatsApp Cloud API, multi-tenant: viven en el mapa de QueTePario.
- Cambios a `GOAL.md` / `GOAL.hash`.
- Generalizar cualquier conclusión a tareas subjetivas sin ítems medidos ahí (OneFlow conserva su alcance).
