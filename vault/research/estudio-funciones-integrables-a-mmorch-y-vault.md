---
title: Estudio — funciones integrables a mmorch y vault
created: 2026-08-10
tags: [research, mmorch, estudio, repo-mining, vault, grafts]
status: verified
confidence: 0.85
---
## Qué es
Exploración 2026-08-07 de Desktop\Estudio (wiki-vault Obsidian mantenido por LLM + SaaS educativa). 14 capacidades relevadas; este es el arbitraje de qué integrar a mmorch/vault.

## Adoptar YA (barato, encaja directo)
1. **Log de operaciones parseable + índice-nunca-drifta** (CLAUDE.md de Estudio §3-4): cada entrada `## [YYYY-MM-DD] <op> | <título>`, set cerrado de ops, `grep "^## \["` como interfaz de query; regla dura de actualizar index/MOC en cada mutación. → convención del vault, cero código.
2. **ACTOR como disciplina de escritura** (general/actor.md): toda nota lleva misión de una línea (`mision:` en frontmatter), un Tronco (compresión de una frase) y al menos una Objeción esbozada. Regla de oro: "si depende del LLM auto-evaluándose, no cuenta". → actualizar vault/templates/research-note.md.
3. **Heurísticas anti-tell de curación** (app/scripts/curate.cjs): rechazar outputs donde la respuesta correcta se delata (más larga/rica que los distractores, 70% prefijo común, auto-referencial). → robar para generate-and-filter de mmorch.
4. **Handoff con comando de verificación + valor esperado** (.claude/handoff.md de Estudio): "cómo verificar si el subagent muerto terminó: `ls wiki/concepts | wc -l`, esperado >100". → convención de session-handoff, mejor que prosa.

## Graft mediano (código, vale ticket propio)
5. **Grading service cross-family** (backend/grade/): 3 piezas que ensemble_verify NO tiene — cost fast-path (un solo juez si todos los ejes son extremos y respuesta corta), snap a niveles discretos 0/.2/.../1 FLAGGEANDO el ajuste, y desacuerdo → tag `desacuerdo_cross_family` + notas por juez preservadas (jamás promediar en silencio).
6. **Máquina de estados de mastery** (app/src/engine/progress/derive-tier.ts, ~50 LOC): nuevo|en-progreso|solido|repasar con ventanas temporales y guard contra "score reciente bueno pero fallas viejas sin resolver". → mapea a retention/resurfacing del vault (qué re-mostrar).
7. **Rubric commit-then-reveal** (wiki-study modo 11): rúbrica oculta escrita ANTES de la respuesta, revelada después, editable por el humano con re-score. → graft para rubric_loop (saca el sesgo de rubric-fitting).

## Ya cubierto / validación mutua
- wiki-sparring YA usa mmorch_adversarial_verify (harvest-no-gate, defensor≠atacante) — Estudio es consumidor real del MCP, nada que importar.
- Verificación determinista (nashpy/sympy/Monte Carlo como ground truth; "sin dato, el Run degenera en sycophancy") = el invariante de mmorch dicho mejor.
- deny Edit/Write sobre *.pdf/pptx/docx en settings (#fuente-inmutable): copiar si el vault ingiere binarios.

## Veredicto cross-family
Items 1-4 son convenciones de un commit. 5-7 son grafts con ticket. El resto contenido local.

## Delta 2026-08-10 (segunda pasada)

Commits nuevos ae29b41/4a23dcb relevados; los 3 grafts medianos de arriba quedaron
SHIPPED en mmorch (pair_verify 643e1ff · derive_tier 348fc27 · commit/reveal 8fd1899).

Adiciones nuevas de Estudio:
1. **Playbook de proveedores alternativos** (backend/ACTIVAR-PROVEEDORES-ALTERNATIVOS.md):
   registry de jueces `(prompt)->str` + `judge_order` como parametro — el seam de swap ya
   quedo cubierto por `verifier_models` de pair_verify. Norma portable: "playbook en vez de
   codigo que no podes testear" (no instalar clients sin credenciales).
2. **Guia de validacion piloto** (docs/validacion-piloto-1-guia.md): patron "guia para
   cuando llegue el input real" como artefacto que gatea trabajo no-verificable. Convencion,
   anotada.
3. **Gate de calidad del grafo DAG** (docs/03-ruta-educativa.md, el mas fuerte): ciclos =
   error de extraccion -> arista DEMOTED a "sugerida, sin confirmar" y excluida del topo
   (degrade-don't-block, con flag), prereqs colgantes -> stub flaggeado, baja confianza ->
   fallback a orden literal flaggeado. SEED para decompose de project_build: hoy re-askea
   ante ciclo; la alternativa degradar-la-arista-con-flag es mas resiliente. No graftear
   a ciegas: cambia semantica del plan — medir primero.
4. HANDOFF.json auto-checkpoint por hook (generador fuera del repo) — esquema interesante,
   ubicar el hook si se quiere adoptar.

