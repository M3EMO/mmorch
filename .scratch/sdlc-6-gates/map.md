# Mapa wayfinder — SDLC de 6 etapas con gates
Label: wayfinder:map · Creado: 2026-09-10 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Todos los proyectos del usuario construyen sus features con el mismo pipeline de 6 etapas (intent → spec → plan → build → test → PR) y gates automáticos: deterministas o checkers sintetizados del `synth_store`. Claude entra solo por escalación y deja siempre una corrección y un chequeo nuevo. mmorch aporta el engine (reemplaza a `/project`); cada repo lleva el contrato (GATE-N.md, plantillas intent/spec/plan, comando de aceptación) como convención scaffoldeable. Es una arquitectura de proyectos, no una feature de Lotus. Se mide por feature: aceptación, USD, minutos, intervenciones, contra 1 etapa.

## Notes

- Decisiones ya tomadas por el usuario (2026-09-10, grilling de charteo): (1) el pipeline reemplaza a project-build, no convive; (2) la escalación es en niveles, automática hasta agotar presupuesto y después avisa — el diseño es el ticket 03; (3) la validación usa el bench congelado de mmorch y features de dogfood de mmorch — corrección 2026-09-11: el ChatBot es otro proyecto, no entra en este mapa; (4) la revisión de la spec antes del build es un gate de juicio FIJO, siempre.
- Evidencia base (leer antes de resolver tickets): `docs/ab-sdlc-2026-09-10/` (A/B/C: engine vs 6 etapas automático vs híbrido; lo mueve el ticket 10), README "Measured", vault "Checkers sintetizados por tipo". Gists: B verde $1.13/22 min/0 intervenciones; C verde $0.16/9 min/2 intervenciones; A rojo tras 4 fixes del engine (commits 58ce334, ea8b775). Las 2 intervenciones de C se reemplazan por gates deterministas (test-compile, regex sobre plan.md).
- Invariantes: especificidad de la cadena = su peor gate; gate LLM sin número medido no entra; sintetizador = deepseek-reasoner, coder = deepseek-v4-pro (medidos); cross-family no decorrelaciona por sí solo (co-fallo cadenas_sin_ab), razonamiento y método sí.
- Skills: `/grill-me` y `/domain-modeling` para grilling; `/research` AFK; `/prototype` para el driver v3. Research al vault global (`mmorch_vault_write`, tag `mmorch`).
- Reglas: resolver tickets es HITL; refutar por default; ambiguo = no concluyente; GOAL.md no se toca; avisar antes de superar ~$15 en un paso.
- Re-scope del usuario (2026-09-10, tras el charteo): el destino NO es integrar a Lotus; es una arquitectura de proyectos para que TODOS sus repos corran así, robustos desde el arranque. Lotus queda como observabilidad opcional. Tickets 11 y 12 salen de esto.
- Este mapa incluye UN prototipo (ticket 04) porque la decisión "¿llega a 0 intervenciones?" solo se responde con un artefacto corriendo. El resto es planificación.

## Decisions so far

- Embudo wayfinder -> SDLC (usuario aprueba 2026-09-15): cada decision ejecutable de un mapa se vuelve una ficha `docs/sdlc/features/<id>.md` (tarea, archivos permitidos, ruta del test); el test de aceptacion se escribe con `/grill-me` junto al humano (modo grill de la etapa 1) y el veredicto humano tambien sale de un grilling, no de un click ciego; `/project` lee la ficha y lanza la corrida. Se aplica al cerrar el ticket 14.

<!-- una línea por ticket resuelto: gist + link -->

- [Features de validación](issues/06-features-de-validacion.md) — 3 del bench congelado (lru-ttl-cache held-out, rate-limiter control 0/3, etl-pipeline interfaces) + 2 de dogfood en mmorch (gen_model en la recursión; gate test-compile en el engine). El ChatBot queda fuera: es otro proyecto con su propio mapa.
- [Prototipo driver v3](issues/04-prototipo-driver-v3.md) — verde con 0 Claude: 9 llamadas, US$0.31, 7.3 min, 0 vueltas (corrida limpia, sin pista ni gate sobreajustado); 4x más barato y 3x más rápido que B; los gates nuevos no rechazaron nada → sin ejercitar hasta las features del 06. Mutación/tests por unidad no incluidos.
- [Cierre del A/B y tabla final](issues/10-cierre-del-ab-y-tabla-final.md) — A rojo a un método de verde (US$0.81, 4 fixes de engine); B verde US$1.13/22 min/0 intervenciones; C verde US$0.16/9 min/2 intervenciones reemplazables por gates. Evidencia en `docs/ab-sdlc-2026-09-10/`.
- [Inventario de gates existentes](issues/01-inventario-gates-existentes.md) — 20 gates en código, todos sin número medido propio; intent, spec y PR tienen cero gates; tres contratos de salida distintos (CheckResult / tuple / int|None) que el ticket 02 debe unificar; el engine no consume synth_store ni coverage/mutation. Detalle: [research/01-inventario-gates.md](research/01-inventario-gates.md).
- [Fix loops en otros agentes](issues/09-fix-loops-en-otros-agentes.md) — nadie reescribe archivos enteros; gate por vuelta solo lint (SWE-agent, Aider), sin revert automático; tope 3 en tres sistemas; stuck-detector y 'gate antes de aplicar' son las ideas para el ticket 03. Detalle: [research/09-fix-loops.md](research/09-fix-loops.md).
- [Contrato de gate](issues/02-contrato-de-gate.md) — función Python + comando; `CheckResult`; `docs/sdlc/gates/`; promoción auto con oráculo 3+1, humana sin oráculo; juicio solo en spec; gate `trazabilidad` por IDs R<n> (2026-09-11). Capture: `brainstorms/2026-09-10-sdlc-6-gates.md`.
- [Testeo modular + regresión](issues/13-testeo-modular-y-regresion.md) — unidad (mutantes) + suite total por unidad + aceptación al final; test-compile fail-closed; umbral de mutación en la ficha, no copiar 80.
- [Escalación por niveles](issues/03-escalacion-por-niveles.md) — 3 vueltas → reasoner×2 → Claude → humano; aviso al pasar a Claude; Claude promueve solo con oráculo+evidencia.
- Validación corrida (2026-09-11/13, `research/07-resultados.md`) — 6/6 verde incluida la held-out, mediana US$0.037, 0 humanos, nivel 3 solo en D2; Claude bloqueó con test 2 veces (D2, ambas reales). El pipeline GANA contra el engine viejo (0/3 en S2). 9 defectos del driver expuestos y convertidos en gates. README "Measured" + vault `sdlc-6-etapas-validacion-6-de-6-verde-2026-09`.
- [Protocolo de comparación](issues/07-protocolo-de-comparacion.md) — 2 brazos (v3 control, v3+Claude fijo producto); repeticiones 1 grande / 3 chicas / held-out x1, cache de worklist apagada; 9 métricas computables; Claude bloquea solo con test que falla; gana = 5/6 + mediana < US$1 + 0 humanos antes del nivel 4; topes por avance (fallos no bajan 2 vueltas, novedad del diff < 10%) + US$3 de fondo, reloj solo por comando.
- [Convención por repo](issues/11-convencion-por-repo.md) — `sdlc.toml` + `accept_cmd` o no entra; scaffold con 4 fichas A/B; intent/PR sin gate; `contract_version` fail-closed. Plantillas desde spec-kit: spec-template, `spec-review.md` desde `clarify`, marca `[P]` (2026-09-11).

- [Integracion al engine](issues/05-integracion-al-engine.md) — `mmorch/sdlc.py` + `build_feature` reemplaza a `project_integrate.build_project` (2 consumidores reales: server_engine, workflow_race); allowlist = techo en `sdlc.toml`, payload solo acota; retiro en 4 pasos con test; `/project` conserva el nombre; la etapa es el checkpoint.

- [Lo subjetivo baja a sintetizado](issues/08-lo-subjetivo-baja-a-sintetizado.md) — veredicto = test + etiqueta + motivo, capturado en el resume (obligatorio); mutation score como gate ejecutable de la etapa 5; checker sintetizado en sombra hasta kappa 0.6; humano en los bordes (A+E+D de la research).
- [Adopcion en repos existentes](issues/12-adopcion-en-repos-existentes.md) — Estudio (TS) y Portfolio (Python) entran con features mergeadas; ChatBot (Java) entra con verificacion en curso; Adepor fuera por decision del usuario; 8 huecos del pipeline corregidos con test (2026-09-17).
- [Hermes como acompañante de la aceptación](issues/14-hermes-acompanante-de-la-aceptacion.md) — Hermes no vive en el pipeline: la skill comun "refutar tests" corre siempre antes del veredicto y propone tests filtrados por gates; plantar Hermes en un repo es opcional; runtime mmorch vs Hermes sale de un banco con oraculo por ejecucion (2026-09-17). Tickets 16-18.
- [Banco de refutación](issues/16-banco-de-refutacion.md) — `mmorch/refutacion.py` + `mmorch cli refutacion`: clasifica un test propuesto por ejecucion (acierto/falsa alarma/neutro/invalido) en worktrees detached; 3 casos en `logs/refutacion/banco.json` (local); verificado 5/5 con tests conocidos (2026-09-17).
- [Skill refutar tests y medición](issues/17-skill-refutar-tests-y-medicion.md) — medido y NEGATIVO: 3 versiones de la skill, 25 corridas con deepseek-reasoner, 0 aciertos y casi todo neutro; los defectos reales se atraparon CON el codigo delante. El ticket 18 se cierra sin construir (2026-09-17).

## Not yet specified

- Intent y PR: sin `GATE-N.md` a propósito (Q16b). No se inventa oráculo.
- Sintetizar y medir (kappa) el checker de aceptacion: `logs/sdlc/veredictos.jsonl` ya tiene 8 veredictos reales (6 aprobados, 2 rechazados, 2026-09-17); con solo 2 negativos el kappa todavia no es medible.
- Retiro del store de bloques cuando ningun lector quede (05 dejo de escribirlo).
- Hermes como emisor de skills por gate. (El ticket 14 ya decidio su rol en la aceptacion: refutador opcional.)
- Señal que justifica plantar un Hermes en un repo (ticket 14 lo dejo como herramienta opcional sin default): espera un caso real.
- Canal movil para aprobar tests desde el telefono (Telegram): el ticket 14 no lo eligio; vuelve si esperar el veredicto frena corridas.
- Exportar los perfiles locales de Hermes en `scripts/claude_config.py`: solo si se planta un Hermes.

- Refutador del test antes de la aprobacion humana: medido negativo en el ticket 17 (0 aciertos en 25 corridas). Vuelve con una hipotesis nueva, por ejemplo un refutador que vea una implementacion; el banco del ticket 16 lo mide en ~12 min.
- Ejecucion en contenedor (Docker) declarada por el repo en `sdlc.toml` (no por RAM libre: cambiaria el entorno entre corridas y el baseline dejaria de ser comparable).

- Recursos visuales (sprites): ticket 15, espera un juego real.

## Out of scope

- Dashboard, WhatsApp Cloud API, multi-tenant: viven en el mapa de QueTePario.
- Cambios a `GOAL.md` / `GOAL.hash`.
- Generalizar cualquier conclusión a tareas subjetivas sin ítems medidos ahí (OneFlow conserva su alcance).
