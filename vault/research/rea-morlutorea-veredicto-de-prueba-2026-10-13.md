---
title: REA (morluto/rea) — veredicto de prueba 2026-10-13
created: 2026-10-13
tags: [research, orchestration, reverse-engineering, mcp, lib-verdict]
status: seed
confidence: medium
sources: ["https://github.com/morluto/rea", "https://www.npmjs.com/package/rea-agents"]
---
## Qué es
REA (`rea-agents` 6.4.0, MIT) es un servidor MCP + CLI de ingeniería inversa. Cubre binarios nativos (vía Hopper/Ghidra/IDA), JS/Electron, .NET, APK, webs, pcaps y firmware. Devuelve "Evidence bundles" con hashes y limitaciones.

## Prueba en la laptop Fedora (Node 22.23)
- Instalación aislada con `npm install --ignore-scripts`: 144 paquetes, 93 MB.
- `rea doctor`: sin Hopper, Ghidra ni IDA, el análisis nativo no corre.
- `inspect-artifact` sobre `bd` (Go, 154 MB): 6 s; devuelve solo hashes y "next_probe: open_binary".
- `analyze-javascript-application` sobre `@modelcontextprotocol/server` (6.4 MB): 79 s, 514 MB RSS, 65 MB de JSON (2596 nodos, 3409 findings).
- `analyze-javascript-application` sobre `obsidian.asar` (27 MB): se trabó >11 min en `pdf.worker.min.mjs` (23/38) con ~7% CPU; cancelado.

## Veredicto
- No sirve para mmorch: mmorch es Python y mide verificadores; REA no aporta oráculo ni checker.
- Sirve ad hoc para entender una app Electron cerrada (Cursor, Claude desktop, Obsidian).
- La salida es enorme: un agente necesita `--token-limit` o `inspect-analysis-view`.
- No correr `rea setup`: modifica la config de los agentes. Usar `npx rea-agents@6.4.0 <cmd>` puntual.
- Nativo exige instalar Ghidra 12.1.x primero.

## A/B ciego: grep vs REA (2026-10-13, n=1 pregunta)
Pregunta: "¿cómo lanza y conecta Claude desktop (2.31226.1, Linux) los servidores MCP locales?". Dos agentes Sonnet ciegos; uno solo con grep, otro solo con REA. Verificación por grep de offsets y strings.

| Métrica | grep | REA |
|---|---|---|
| Tiempo | 3.4 min | 16 min (6 min un solo análisis) |
| Tokens del agente | 140k | 155k |
| Subpreguntas respondidas (de 5) | 5 | 1 completa (IPC), 1 parcial (extensiones) |
| Verificación | 13/13 offsets reales | strings reales; 1 falsa (".mcpb ausente": está en 9 archivos); 1 inferencia errada (tool calls vía `mcpCallTool`; el camino real es un MessagePort) |

- REA no expone strings literales del bundle; por eso no encontró rutas de config ni de logs.
- `recover-javascript-sources` exige Wakaru; `search`/`decompile` solo sirven para nativo; `inspect-analysis-view` tiene el input sin documentar.
- Aporte propio de REA: inventario de ~1300 canales IPC con dirección, miembros del contextBridge y flags de sandbox por ventana. Grep también los encuentra si alguien los busca.

Veredicto revisado: para JS/Electron minificado, grep dirigido por la pregunta gana. REA queda como herramienta de inventario. Lo no probado: nativo con Ghidra, y REA con Wakaru instalado.
