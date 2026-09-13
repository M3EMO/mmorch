---
title: SDLC de 6 etapas con gates — validación 6/6 verde (2026-09-11/13)
mision: ¿Un pipeline de 6 etapas con gates deterministas y Claude fijo solo como revisor gana contra el engine viejo de mmorch, medido en USD, minutos y escalaciones?
status: verified
confidence: 0.9
verifier: ejecucion (tests de aceptacion congelados del bench + suite total de mmorch por nombre)
tags: [research, mmorch, sdlc]
sources: []
created: 2026-09-13
---

## Tronco

Seis features (3 del bench congelado, 2 de dogfood sobre mmorch, 1 held-out) quedaron verdes con mediana US$0.037 por feature, 0 humanos y un solo caso que subio al nivel 3; el engine viejo tenia 0/3 en la misma task de control.

## Qué es

Driver `driver_py.py` (`.scratch/sdlc-6-gates/research/04-driver-v3/`): spec -> revision de spec por Claude (<= 5 preguntas, de spec-kit `clarify`) -> plan -> build -> test -> revision del diff por Claude -> PR. Escalera: fix loop 3 -> deepseek-reasoner x2 -> Claude (modo edit) -> humano. Coder deepseek-v4-pro, escritor deepseek-reasoner. Protocolo y criterio de gana: ticket 07 del mapa `sdlc-6-gates`.

## Evidencia / mecanismo

| feature | corridas | verde | USD mediana | min | nivel max | Claude bloqueo con test |
|---|---|---|---|---|---|---|
| S2 rate-limiter (control, engine viejo 0/3) | 5 | 5/5 | 0.03 | 2-3 | 0 | 0 |
| D3 detector de atasco (mmorch) | 1 | 1/1 | 0.04 | 34 | 0 | 0 |
| S3 etl-pipeline (3 modulos cruzados) | 1 | 1/1 | 0.04 | 6 | 0 | 0 |
| D2 gate test-compile en el engine (mmorch) | 1 | 1/1 | 0.84 | ~60 | 3 | 2, ambos reales |
| S1 lru-ttl-cache (held-out, una vez, al final) | 1 | 1/1 | 0.02 | 4 | 0 | 0 |

D1 ya estaba verde en el baseline: su test queda como regresion. Detalle por corrida: `research/07-resultados.md`.

Mecanismo que sostiene el numero: cada gate es determinista y cuesta US$0 (tokens del contrato, trazabilidad R<n> spec<->plan<->tests, allowlist del plan, py_compile, collect-only, regresion por unidad, topes por avance, alcance de cada pasada de Claude, lint, suite total por nombre contra baseline medido en el worktree). Claude entra fijo dos veces y bloquea SOLO si deja un test que falla.

## Aplicable a mmorch

- El pipeline reemplaza a `project-build` (decision del mapa). Ticket 05 decide la migracion; ahora tiene evidencia.
- Las 9 fallas del driver que las corridas expusieron son gates nuevos, no prompts: un-archivo, valla exterior, test-compile en el fix loop, alcance, lint con escalera, baseline por sha, recorrida aislada de flaky, archivos del plan por item, reescritura del plan.
- La revision fija de Claude atrapo 0 defectos en 8 corridas chicas y 2 reales en la dificil. Se queda como seguro; su costo es cupo, no USD.

## Objeciones

- n chico: 9 corridas, 5 features. La held-out es una sola task chica. Falta una feature grande sobre repo real ajeno a mmorch.
- El costo de D2 (US$0.84) fue un defecto del driver; sin ese defecto no se midio. Una corrida limpia de D2 daria el numero honesto.
- La suite total de mmorch trae 5 rojos previos y 3 flaky bajo carga: el gate compara por nombre, no exige verde absoluto.
- Todo es checkeable con oraculo. Nada de esto se generaliza a tareas subjetivas (ticket 08 sigue abierto).

## Veredicto cross-family

- passed: si (ejecucion, no juicio LLM). confidence 0.9. Objecion principal: n chico y D2 contaminado por el defecto del driver.

## Links

- [[sdlc-6-gates-herramientas-y-prácticas-de-industria-por-ticke]]
- [[suite-unidad-regresión-mutación-práctica-documentada-ticket-]]
- README mmorch, seccion Measured "6-stage pipeline on the validation set".
