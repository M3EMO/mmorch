---
title: Desglose medido del contexto de arranque de Claude Code (2026-11-26)
created: 2026-11-26
tags: [research, orchestration, vault-y-compresor, arranque]
status: seed
confidence: media
sources: [transcripts ~/.claude/projects/*/*.jsonl (campo usage); API count_tokens de Anthropic con modelo claude-haiku-5-5; ~/.claude/settings.json; ~/.claude/plugins/cache/ecc]
---
## Total real
Se tomó el primer mensaje del assistant con usage > 0 (input + cache_creation + cache_read) en sesiones recientes.
- Proyecto orchestration (11 sesiones): 64.3k a 69.2k tokens, mediana 66.8k. Una sesión grande dio 91k y una 109k (ya tenía turnos).
- Otros proyectos (Estudio, Proyectos, mf-moe, BetDeportivo): 52k a 91k.

## Desglose por fuente (contador count_tokens de Anthropic)
| Fuente | Tokens | Controlable sin editar ECC |
|---|---|---|
| ~/.claude/CLAUDE.md | 846 | Sí |
| CLAUDE.md de proyecto | 1767 | Sí |
| AGENTS.md de proyecto (importado) | 2882 | Sí |
| MEMORY.md | 868 | Sí |
| Skills ECC (314, nombre y descripción) | ~29.7k | Sí: skillOverrides o enabledPlugins en settings.json |
| Comandos ECC (98) | ~5.6k | Sí, igual (el plugin entero) |
| Agentes ECC (71) | ~7.8k | Solo apagando el plugin completo; no hay palanca por agente |
| System prompt, esquemas de tools nativos, instrucciones MCP, skills propias, nombres de tools diferidos | ~17k (resto) | No (system prompt y tools nativos); MCP sí con claude mcp |

## Confiabilidad
- Total: alta (medido en transcripts).
- Archivos .md: alta en tokens del contador. Se midió con el tokenizador de claude-haiku-5-5, que puede diferir algo del modelo de la sesión.
- Skills, comandos y agentes ECC: media. Se reconstruyó el texto de la lista desde el frontmatter (descripción cortada a 250 caracteres en skills); el formato real del listado puede variar.
- Resto (~17k): estimación por diferencia, no medida directa. Suma de lo medido: ~49.5k contra 66.8k de total.
- settings.json no tiene skillOverrides hoy. enabledPlugins tiene ecc y ponytail en true.
