---
title: "Setup de Claude Code: evidencia medida sobre CLAUDE.md, skills y archivos viejos (2026-11-26)"
created: 2026-11-26
tags: [research, orchestration, research, claude-code, setup, context-files, agents-md, lacuna]
status: seed
confidence: "media: números de abstracts y del post de Google; no leí los PDF completos"
sources: ["https://arxiv.org/abs/2602.11988", "https://arxiv.org/abs/2610.30571", "https://arxiv.org/abs/2611.09802", "https://developers.googleblog.com/agent-distraction/", "https://www.trychroma.com/research/context-rot", "https://arxiv.org/abs/2507.11538", "https://arxiv.org/abs/2603.29919", "https://arxiv.org/abs/2505.03275", "https://code.claude.com/docs/en/memory", "https://code.claude.com/docs/en/skills", "https://code.claude.com/docs/en/permissions", "https://lacuna.tiptreesystems.com"]
---
# Setup de Claude Code: evidencia medida

Fecha: 2026-11-26. Método: un subagente buscó en la web y en Lacuna. Alien no aplica: su corpus es de papers de ML hasta 2025.

## Medido

- Gloaguen 2026 (arXiv 2602.11988): un AGENTS.md generado por LLM sube el costo 20-23% (p<0,001) y no mejora el éxito. Uno escrito a mano suma +2,4 pp sin significancia.
- Xu y Wu 2026 (arXiv 2610.30571): con un archivo de instrucciones viejo, 90,1% de las decisiones sigue al archivo. Una oración que da precedencia al código lleva el seguimiento del código a 91,4% y los tests verdes a 100%.
- Mehtiyev 2026 (arXiv 2611.09802): un comando de test viejo suma 33% de llamadas. Ninguna corrida reparó el archivo sola.
- Google, 2026-11-06: 13% de las sesiones lee un TODO de bug. 83% de las desviaciones no se pidió. Una desviación cuesta 70% más pasos, 77% más tokens y el doble de espera. Los humanos aceptaron más de 95% de los borrados propuestos con chequeo de historial.
- Chroma 2025: los 18 modelos rinden peor con más contexto. Un distractor ya baja el rendimiento.
- IFScale 2025: el mejor modelo llega a 68% con 500 instrucciones.
- SkillReducer 2026: más de 60% del cuerpo de un skill no es accionable. Comprimir ahorra 43% de tokens.
- RAG-MCP 2025: 13,6% de acierto con el catálogo completo de tools y 43,1% con recuperación.

## Doc oficial (opinión del fabricante)

- CLAUDE.md global guarda preferencias de todo proyecto. El de proyecto guarda hechos de ese repo.
- La meta es menos de 200 líneas por archivo. Un `@import` no ahorra contexto.
- Una regla dura va a un hook o a `permissions.deny`, no a CLAUDE.md.
- `Bash(python *)` deja a un script leer o escribir sin pasar por las reglas Edit.
- Ante dos reglas opuestas, Claude puede elegir cualquiera.

## Lacuna

- La búsqueda de papers sirvió: trajo Gloaguen, Mehtiyev, SkillReducer y Harness-IF, que la web no trajo primero.
- La vista `/directions` devolvió 2077 resultados genéricos y no sirvió.

## Pendiente

- Leer el PDF de Xu y Wu para copiar la oración exacta.
