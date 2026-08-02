# Inventario de research local en los repos activos

Type: research
Status: resolved

## Question

¿Qué documentos de research/investigación viven hoy LOCALES en cada repo activo
(Desktop\Claude\* y ~/.claude/orchestration), con path, tamaño, fecha y una
línea de qué es? Sin esto no se puede decidir qué migrar ni el criterio
vivo-vs-histórico. Excluir: código, specs de GSD (.planning), node_modules,
artefactos generados.

## Answer

Inventario completo (subagent 2026-08-02): **~38 candidatos, ~253 KB**. mmorch domina
por un orden de magnitud (~158 KB fuera del vault en 17 docs); Lotus ~35 KB (5 docs,
solo 1 vivo: el brainstorm de intent del producto); experimentoTrabajo ~9.5 KB (1 doc
histórico). bitterbot-desktop y caveman-upstream son clones upstream — excluidos.

Hallazgos clave:
1. **3 docs ya duplicados byte-idéntico** entre docs/ y vault/research/ (intuition-layer,
   fable-workflow, paperclip-grafts) — migración fue copia sin pointer → docs/ va a driftar.
2. **El research de mayor señal sigue FUERA del vault**: brainstorms/2026-06-08-mmorch-
   ideal-vision.md (18.7 KB, ancestro de GOAL+SELF-EVOLUTION-PLAN), cooperative-workflow.md,
   sandbox-executor.md (veredicto REJECTED), HERMES-IDEAS.md, ALGORITHMS-MAP.md,
   flywheel/RESULTS.md (único benchmark real), WEIGHTS.md.
3. La premisa "varios repos" es realmente **2 repos activos + mmorch**.

Tabla completa por repo con verdictos vivo/histórico: ver transcript del subagent
(sesión 4d6cf9a4, 2026-08-02). Vivos fuera del vault: 8 en mmorch root/docs,
1 en Lotus. Históricos: audits/roadmaps auto-generados, prompts Kimi v1/FINAL,
superpowers SDD ejecutados.
