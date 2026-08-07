---
title: Babel para prompts — refutado por medicion
created: 2026-08-07
tags: [research, mmorch, babel, prompts, autoresearch]
status: refuted
confidence: 0.95
---
## Qué es
Corrida autoresearch 2026-08-03 (A/B, journal logs/ar_babel_prompt.qrf): ¿un system-prompt babel-comprimido rinde igual que el original contra la batería edge-heavy congelada?

## Evidencia / mecanismo
- Original 298 chars: score 0.8889. Babel: EL ENCODER EJECUTÓ el prompt en vez de comprimirlo (devolvió código Python — confusión instrucción/dato). Fix: payload delimitado <<<>>> (commit fe58e22).
- Con delimitadores, texto de 298 chars sale destruido (16 chars) → pre-filtro MIN_CHARS=3000 implementado en ingest.
- El snippet-basura como system-prompt EMPATÓ (0.8889) al prompt optimizado: la batería no discrimina prompts en el plateau — techo del scorer, no del prompt.

## Aplicable a mmorch
Babel = SOLO documentos de contexto (research, notas), jamás instrucciones. Para seguir optimizando prompts hace falta batería más dura, no más rondas.

## Veredicto cross-family
Verdad de ejecución directa (scorer congelado), sin LLM-judge.
