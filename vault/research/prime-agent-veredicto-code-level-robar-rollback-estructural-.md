---
applies_to:
- OS propio
- orchestration
---
Code-read 2026-08-14 (agente dedicado sobre clon shallow). prime-agent = "self-improving RLM agent" de PrimeIntellect: kernel IPython persistente como único tool + harness state durable (prompt/memory/skill/subagent) refinado por LLM.

## Veredictos

1. **Continual Harness //refine — ROBAR PARCIAL.** `refinement.ts` (1017 líneas, autocontenido): harness_state.json versionado, edits CRUD chicos propuestos por LLM sobre la trayectoria (80K chars). Su "evidence-backed" es BLANDO (rationale cita la conversación, sin métrica) — más débil que el evolve/hillclimb de mmorch. Lo superior: **rollback estructural** — cada edit guarda before/after snapshot (762-779) y `rollbackProposal` (804) invierte mecánicamente sin LLM desde history JSONL; + gate barato `reviewAutoRefine` (949) que decide si refinar antes de gastar; + detección de conflicto plan-vs-apply (726-740).
2. **RLM / prompt-as-variable — IGNORAR.** No es mecanismo, es la arquitectura (kernel persistente; variables sobreviven compaction). Adoptarlo = cambiar el runtime. El equivalente barato (blobs en SQLite + handles) mmorch ya lo tiene.
3. **rlm() subagents — ROBAR UNA IDEA.** Fire-and-forget con admission handle + **subagentes retenidos re-consultables** (`agent_message.send(receiver_role="child", name=...)` con contexto intacto post-compaction). fan_out/orchestra es batch-síncrono; graft: retener conversación por brazo en SQLite + `follow_up(name, msg)`.
4. **Skills ejecutables — ROBAR EL CONTRATO.** Skill = dir con SKILL.md + pyproject instalado editable → invocable `await skill(...)`. La "creación automática" es el mismo refine con schema validation (`validateEdit` 680-703: reference {type:python, import, callable} + arguments). session_skills mina playbooks pero no ejecuta: graft = campo `reference {module, callable, args_schema}` opcional en playbooks + validación copiable de 680-703.
5. **Goals/heartbeats/autonomous — YA-LO-TENEMOS.** Su goals+autonomous ≈ loop-cerrado + autoresearch (gate frozen + budget). Detalle robable menor: claim-before-deliver + coalescing de ticks perdidos en schedules.

## Top-3 grafts (valor/esfuerzo)

1. Rollback estructural de refinements → aplicar a evolve/close-loop (~100 líneas, alto valor: revertir un finding malo hoy es manual).
2. Playbooks ejecutables (contrato reference+arguments validado) → cierra el gap minar→ejecutar de session_skills.
3. Review-gate barato pre-persistencia (1 call DeepSeek shouldRefine) → filtra ruido del ingest de sesiones. Trivial.

**NO adoptar:** el runtime RLM completo (IPython + daemon + ZeroMQ, acoplado a host TypeScript). Los 3 grafts capturan ~todo el valor. Cargados como candidatas cand-2026-08-14-01..03 en vault/roadmaps/candidatos.md (dogfood del loop F4).
