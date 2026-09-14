---
title: mmorch — módulos sin uso, medición 2026-09-14
mision: ¿Qué módulos de mmorch no alcanza ninguna entrada real ni registran uso en 90 días, para podar por medición y no por opinión?
status: verified
confidence: 0.7
verifier: analisis estatico (ast, alcance desde entradas) + ledger metrics.jsonl y mcp_calls.jsonl (90 dias)
tags: [research, mmorch]
sources: []
created: 2026-09-14
---

## Tronco

24 de 136 modulos no se alcanzan desde ninguna entrada (mcp_server, server, nightly, cli, scripts); 14 mas se alcanzan pero no registran uso en 90 dias ni tienen test; el resto vive.

## Qué es

Tres evidencias por modulo: (1) alcance estatico por imports desde las entradas reales; (2) hits en `logs/metrics.jsonl` (pattern) y `logs/mcp_calls.jsonl` (tool) en 90 dias; (3) si algun test lo importa. Limite: el analisis estatico no ve imports dinamicos (`importlib`), y los hits runtime solo aplican a modulos que llaman modelos o tools.

## Evidencia / mecanismo

No alcanzados desde entradas (candidatos fuertes; loc; tests):

| modulo | loc | tests | nota |
|---|---|---|---|
| workflow_store | 346 | si | |
| context_blocks | 244 | no | diseño "cooperative workflow", fase A |
| adjudicate | 197 | si | |
| pty_session | 173 | si | server_pty lo importa lazy; server_pty tampoco tiene uso |
| plugins + plugin_worker | 230 | no | plataforma G11; nadie la cablea (D6 la endurecio igual) |
| shadow_prior | 159 | si | |
| workflow_spec | 148 | si | |
| predict | 118 | si | |
| feedback_trace | 112 | no | |
| tournament | 109 | si | evolucion de workflows, diseño |
| job_graph | 103 | no | |
| synth_store | 103 | si | lo usan scripts de ablacion, no el engine |
| code_embedder | 102 | no | |
| bucketrank | 100 | si | |
| factory | 96 | si | |
| chat_store | 81 | si | |
| durable_runs | 81 | no | |
| weights | 80 | si | |
| schedule | 66 | si | |
| megasource | 59 | si | |
| innovate | 50 | si | 1 hit runtime |
| effort | 37 | si | |
| loop | 61 | si | 4173 hits: el pattern `loop_propuestas` lo nombra; import dinamico probable (falso positivo del estatico) |

Alcanzados pero sin uso en 90 dias y sin test: evolve_findings (253), workflow_race (224), bughunt (201), workflow_engine (161), workflow_evolve (160), bursts (143), server_pty (101), arbitration (93), portability (88), minds (84), gate_policy (65), server_fleet (52). `lang` y `textutil` son utilidades sin llamadas a modelos: el hit runtime no aplica.

Uso real dominante (90 dias): ablation_paired 6504, loop_propuestas 4100, project_integrate 1824, distill 1378, adversarial_verify 1841, code_review 547, fan_out 511, babel 231. Tools MCP: budget_status 164, record_outcome 78, review_code 40, adversarial_verify 27, vault_write 23.

## Aplicable a mmorch

- Poda en dos pasos: primero un test de capas que prohiba importar los 24 no alcanzados desde el engine (ratchet, como `test_no_museum`); despues borrar por lote con PR del pipeline, un modulo por feature, con la suite total como gate.
- Los 12 "alcanzados sin uso" son la decision de producto: server_pty/fleet/gate_policy (plataforma), evolve_findings/workflow_race/workflow_evolve/bursts (evolucion), arbitration/minds (cognicion). Se podan o se miden; no se dejan.
- D6 endurecio `plugins`, un modulo sin caller: la medicion tendria que haber ido ANTES de la robustez. Orden corregido para los modulos que quedan.

## Objeciones

- Import dinamico: `loop` muestra que el estatico falla; revisar los 24 con `grep importlib|import_module` antes de borrar.
- 90 dias de ledger cubren solo llamadas a modelos y tools MCP; un modulo determinista puede usarse sin dejar rastro. El test de capas + suite total es el gate real.

## Veredicto cross-family

- passed: parcial. confidence 0.7. Objecion principal: falsos positivos por import dinamico.

## Links

- [[sdlc-6-etapas-validacion-6-de-6-verde-2026-09]]
- `.scratch/sdlc-6-gates/research/07-resultados.md`
