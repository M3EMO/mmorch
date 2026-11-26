# Desglose medido del contexto de arranque
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Medí cuántos tokens aporta cada fuente al arranque de una sesión de Claude Code en esta laptop. Las fuentes son el system prompt, cada CLAUDE.md, la lista de skills, la lista de agentes, las instrucciones de MCP y los esquemas de tools. Usá el contador real de Anthropic o transcripts reales, nunca una estimación por caracteres. Marcá qué fuente controla el usuario sin editar ECC.

## Answer

Total real del arranque en este repo: mediana 66.8k tokens (rango 64.3k a 69.2k, 11 sesiones).

| Fuente | Tokens | Controlable sin editar ECC |
|---|---|---|
| CLAUDE.md global | 846 | Sí |
| CLAUDE.md de proyecto | 1767 | Sí |
| AGENTS.md de proyecto | 2882 | Sí |
| MEMORY.md | 868 | Sí |
| Skills ECC (314) | ~29.7k | Sí: `skillOverrides` o `enabledPlugins` |
| Comandos ECC (98) | ~5.6k | Sí, con el plugin entero |
| Agentes ECC (71) | ~7.8k | Solo apagando el plugin; sin palanca por agente |
| Resto (system prompt, tools nativos, MCP, skills propias) | ~17k | Casi nada; MCP sí con `claude mcp` |

Confiabilidad: el total es medido. Los .md usan el contador real (tokenizador de haiku-5-5). Skills, comandos y agentes son reconstruidos del frontmatter. El resto es estimación por diferencia. `skillOverrides` no existe hoy en settings.json.

Nota del vault: "Desglose medido del contexto de arranque de Claude Code (2026-11-26)".
