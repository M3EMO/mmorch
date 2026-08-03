# Frontera vault vs memoria auto de Claude Code

Type: grilling
Status: resolved

## Question

Claude Code tiene su memoria auto por proyecto (~/.claude/projects/<p>/memory/).
¿Qué va ahí y qué va al vault? ¿Se linkean, se duplican, o una absorbe a la otra?

## Answer

Grilling 2026-08-03: **Regla por contenido + links** — cross-proyecto y curado
(research, veredictos, benchmarks, design-rationale) → vault. Sesión/proyecto
(preferencias, estado de trabajo, feedback del usuario) → memoria auto de Claude.
La memoria auto PUEDE linkear notas del vault por path; contenido no se duplica.
Ninguna absorbe a la otra.
