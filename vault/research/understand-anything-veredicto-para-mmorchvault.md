---
title: Understand-Anything — veredicto para mmorch/vault
created: 2026-08-10
tags: [research, mmorch, repo-mining, knowledge-graph, vault]
status: seed
confidence: 0.8
sources: [https://github.com/Lum1104/Understand-Anything]
---
## Qué es
Plugin de Claude Code (Egonex/Lum1104, MIT): pipeline multi-agente que construye un knowledge graph del codebase (`.ua/knowledge-graph.json`) + dashboard interactivo (tours guiados, búsqueda semántica, diff-impact, vistas por capa/dominio). Incluye `/understand-knowledge`: parser DETERMINISTA de wikis patrón-Karpathy (wikilinks + index.md) + agentes LLM que agregan relaciones implícitas, entidades y claims → grafo force-directed.

## Evidencia / mecanismo
Leído del clon (2026-08-07): 9 skills, 10 agentes. La arquitectura del `/understand-knowledge` es dos capas: parse determinista primero (scan-manifest.json con nodos/edges de wikilinks), LLM después solo para lo implícito — mismo principio que mmorch (el LLM propone sobre base determinista).

## Aplicable a mmorch
- **Codebase graph: REDUNDANTE** — codegraph MCP ya da símbolos/callers/impact con queries sub-ms; lo único nuevo es el dashboard visual y los tours.
- **Vault graph: PREMATURO** — el vault tiene ~12 notas; el graph view nativo de Obsidian ya muestra los wikilinks gratis. El plugin pagaría con >50 notas cuando las relaciones implícitas (que Obsidian no ve) tengan masa crítica.
- **Patrón robable** (no el plugin): capa determinista de parse + capa LLM de relaciones implícitas, versionadas por separado. Ya es la arquitectura de babel/MOC.

## Veredicto cross-family
Seed con trigger: revisar cuando el vault pase 50 notas o cuando haga falta onboarding visual de mmorch (98 módulos). No adoptar hoy.
