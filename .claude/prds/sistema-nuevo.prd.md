# Sistema nuevo: reemplazo de mmorch y su SDLC, con REA y CAB

## Problem
El SDLC de mmorch es lento y no cumple lo que el usuario necesita. El usuario da tareas y los proyectos quedan a medias. El sistema tampoco aprende de sus corridas. El costo de no resolverlo: TBD — needs validation via respuesta del usuario.

## Evidence
- Medido: memora con el SDLC llega a 21/61 de contrato y 20/48 de oráculo oculto, con 4 intervenciones humanas y US$2,92 (fuente: `docs/contexto/medido-mmorch.md`). El usuario confirma que cuenta como evidencia.
- Medido: brasa con el SDLC tiene 5 intervenciones humanas y US$3,37, sin terminar (fuente: `docs/contexto/mapa.md`). El usuario confirma que cuenta como evidencia.
- Observación del usuario: el SDLC es lento y no cumple. Assumption — needs validation via tiempo y tasa de éxito medidos en tareas reales.
- Observación del usuario: los proyectos quedan a medias después de dar tareas. Assumption — needs validation via conteo de tareas terminadas y abandonadas.
- Observación del usuario: el sistema no aprende. Assumption — needs validation via método sin definir (el usuario no lo tiene claro).

## Users
- **Primary**: desarrollador único que construye software con agentes de Claude Code y que mide el costo en cupo y USD.
- **Not for**: multi-usuario (fuera de alcance en `GOAL-mmorch.md`).

## Hypothesis
We believe **ECC, integrado con Root, Graft, REA y CAB** will **resolver mmorch y el SDLC** for **desarrollador único**.
We'll know we're right when **se integran las cuatro piezas y un proyecto llega a calidad top** (TBD — needs validation via definición de "los 3" y de "calidad top").

## Success Metrics
Propuesta del mapa (`docs/contexto/mapa.md`):

| Metric | Target | How measured |
|---|---|---|
| Contrato memora desde bootstrap con ECC y Haiku 5.5 | 61/61 | Oráculo de contrato del banco |
| Contrato brasa desde bootstrap con ECC y Haiku 5.5 | 53/53 | Oráculo de contrato del banco |
| Oráculo oculto | 48/48 (memora) y 15/16 (brasa) | Oráculo oculto del banco |
| Intervenciones humanas durante la construcción | 0 | Conteo en el log de la corrida |
| Costo de construcción | TBD — needs validation via respuesta del usuario (cupo) | Costo en cupo y USD por corrida |
| Calidad de un proyecto | TBD — needs validation via definición de "calidad top" | Por definir |
| Invariantes de `GOAL-mmorch.md` preservados | Sin pérdida sin decisión del usuario | Revisión contra el GOAL |

## Scope
**MVP** — Un mejor producto de Brasa: más completo y más barato. (Interpretación del texto del usuario; confirmar.)

**Piezas**
- **Root**: reemplaza a mmorch. Incluye Route, Oracle, Observe y Track.
- **Graft**: reemplaza al SDLC.
- **REA**: se suma al pipeline. Servidor MCP de ingeniería inversa de `morluto/rea` (MIT).
- **CAB** (Code Agent Builder): por construir.
- Todo se construye con ECC.

**Builder, memoria y ruteo** entran en el rediseño. El banco, el vault y el resto de mmorch también se rediseñan.

**Out of scope**
- Multi-usuario — fuera de alcance en `GOAL-mmorch.md`.
- Acciones del mundo real sin gate humano — fuera de alcance en `GOAL-mmorch.md`.
- Sobrescribir el repo de mmorch — descartado por decisión del usuario (opción B).
- Lo que queda afuera del alcance: TBD — needs validation via respuesta del usuario.

## Decisiones tomadas
- Repo: el sistema nuevo vive en rama aparte, `sistema-nuevo`, con worktree propio. mmorch se retira al final, cuando el sistema nuevo pase las métricas.
- Cada integración se planea en su propia sesión con `/plan`.
- Todo se construye con ECC.
- Modelos por rol (decisión del usuario, 2026-10-27): "El plan debe crearlo opus, sonnet organiza a los builders, haiku debe ser el builder."
- Producto (decisión del usuario, 2026-10-27): Root, Graft, ECC y REA forman un producto completo, con ECC entero. ECC, Graft y REA se reconstruyen después.
- Herencia de mmorch (decisión del usuario, 2026-10-27): las mediciones de "Measured" valen como evidencia, pero el método queda en revisión. Se conserva el hash del GOAL. `metrics.jsonl` se importa en solo lectura. El código arranca de cero.
- Root (decisión del usuario, 2026-10-27): Root es un evaluador de alto nivel (seguridad, diseño, eficiencia, estructura, arquitectura) y aprende. Route balancea DeepSeek y Anthropic contra el cupo. Lenguaje: Rust. Root maneja TOML, Dockerfile y YAML. El vault queda fuera de Root porque el sistema empieza de cero.
- Binario: Root es el crate `root` en Rust, y su binario se llama `r00t` (`.claude/plans/root.plan.md`, "Decisiones del usuario").
- Dimensiones: el Oracle evalúa nueve dimensiones y actúa como project manager que entrega un informe (`.claude/plans/root.plan.md`, Tareas 9 a 13b).
- Rúbricas: cada dimensión se puntúa con una rúbrica que cita su estándar, y Root no inventa criterios (`.claude/plans/root.plan.md`, decisión 12).
- Ficha del spec: cada SPEC lleva una sección `## Ficha` obligatoria, y sin ella no hay veredicto (`.claude/plans/root.plan.md`, decisiones 18 y 20).
- Repeticiones: cada suite corre varias veces, y cada diferencia lleva una causa y un origen (`.claude/plans/root.plan.md`, decisiones 16 y 19).
- Route: los topes de cupo, API y nube limitan la elección, y un tope lleno registra un `Gate` (`.claude/plans/root.plan.md`, decisión 15).
- Aprende: la política de Route se recalcula desde las filas y los eventos (`.claude/plans/root.plan.md`, Tarea 15).
- Red: evaluar no usa red, salvo en la fase de descarga de dependencias (`.claude/plans/root.plan.md`, decisión 17).
- Vault: el vault queda fuera de Root, y el nombre `mmorch` solo etiqueta los datos importados (`.claude/plans/root.plan.md`, decisiones b3 y b4).

## Delivery Milestones
<!-- Business outcomes, not engineering tasks. /plan turns each into a plan. -->
<!-- Status: pending | in-progress | complete -->

| # | Milestone | Outcome | Status | Plan |
|---|---|---|---|---|
| 1 | Root: reemplaza el banco y el ruteo de mmorch | Route, Oracle, Observe y Track corren desde el bootstrap del banco | in-progress | `.claude/plans/root.plan.md` |
| 2 | Graft: reemplaza el SDLC | El SDLC de Graft completa tareas sin intervención humana | pending | — |
| 3 | REA: se suma al pipeline | Un paso del pipeline usa REA y deja evidencia verificable | pending | — |
| 4 | CAB: Code Agent Builder | El sistema construye agentes de código desde una especificación | pending | — |
| 5 | Retiro de mmorch | Mmorch se retira sin perder mediciones ni invariantes | pending | — |

Orden y dependencias entre milestones: TBD — needs validation via respuesta del usuario.

## Open Questions
- [ ] ¿Qué significa "los 3" en la hipótesis? ¿ECC, mmorch y SDLC, o Root, Graft y CAB?
- [ ] ¿Qué significa "calidad top" y cómo se mide?
- [ ] ¿Cuál es el nombre del sistema completo? Root y Graft son piezas.
- [ ] ¿Qué significa "el sistema no aprende" y cómo se valida? (el usuario no lo tiene claro)
- [ ] ¿Qué parte del funcionamiento no cierra? (aclaración pendiente)
- [ ] ¿Qué queda afuera del alcance? (el usuario no lo tiene claro)
- [ ] ¿Qué se conserva de mmorch? (el usuario no lo tiene claro)
- [ ] ¿Qué cuenta como intervención humana? (el usuario no lo tiene claro)
- [ ] ¿Cuánto cupo acepta para construir el sistema entero? (el usuario no lo tiene claro)
- [ ] ¿Cómo convive el sistema nuevo con mmorch mientras dura la migración?
- [ ] ¿Por qué ECC entero, si el veredicto recomienda no instalar ECC entero y usar solo el paso del planner?
- [ ] ¿Sobre qué apps se usa REA? ¿Qué licencias y términos aplican a esas apps?
- [ ] ¿Cuál es el orden de las sesiones de integración? ¿Qué depende de qué?

## Risks
| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Haiku 5.5 no alcanza los números de Sonnet | TBD — needs validation via medición de Haiku antes de Root | Alta | Medir primero (mapa, ticket 03) |
| "Todo con ECC" contradice el veredicto, que recomienda no instalar ECC entero | Alta (conflicto documentado) | Alta | Decisión del usuario antes de Root |
| Hooks de ECC escriben comandos completos en `~/.claude/`, incluso secretos | Confirmada (veredicto ECC, 2026-10-26) | Alta | Desactivar esos hooks con `ECC_DISABLED_HOOKS` en cada corrida |
| ECC solo aconseja su proceso y el agente lo salta | Confirmada (mapa: salteó `/orch-build-mvp`) | Alta | Gates como hooks que bloquean, no como consejo (mapa, ticket 06) |
| REA modifica la configuración del agente al instalarse | Confirmada (nota del orquestador) | Alta | Gate humano antes de instalar. Aislar en su propia sesión |
| REA se usa sobre apps de terceros: riesgo legal o de licencia | TBD — needs validation via respuesta del usuario | Alta | Definir alcance y apps permitidas antes de usar REA |
| Cuatro integraciones a la vez: el alcance se desborda | Media | Alta | Una sesión y un milestone por integración |
| Retirar mmorch pierde mediciones o invariantes | Media | Alta | Respaldar mediciones y `GOAL.md` antes de retirar. Revisar contra el GOAL |
| El nombre mmorch aparece en cientos de archivos y configuraciones | Confirmada | Media | Inventario de referencias antes del retiro |

---
*Status: DRAFT — requirements only. Open questions pending. Implementation planning pending via /plan, one session per integration.*
