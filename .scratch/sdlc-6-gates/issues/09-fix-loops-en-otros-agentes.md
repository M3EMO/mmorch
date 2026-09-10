# Fix loops en otros agentes
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Cómo hacen el fix loop dirigido y la escalación otros agentes de código publicados (OpenHands, SWE-agent, Aider, Cursor agent): ¿el modelo elige archivos? ¿hay gate de compile por vuelta y revert? ¿cuántas vueltas? ¿qué pass@1 publican con y sin loop? ¿cómo escalan a humano? Fuentes primarias. Salida: `research/09-fix-loops.md` con tabla comparativa y 3 ideas robables para el ticket 03.

## Answer

- Ninguno reescribe archivos enteros. Todos editan dirigido: rango de líneas (SWE-agent), string replace (OpenHands), search/replace (Aider), diff + shadow workspace (Cursor).
- Gate por vuelta solo en SWE-agent (lint, rechaza la edición antes de aplicar; +3 pts Lite) y Aider (lint default, tests opt-in). Nadie hace revert automático "si empeora"; el revert es humano (`/undo`, checkpoints).
- Tope corto = 3 en Aider (`max_reflections`), Bugbot Autofix (3 intentos/PR) y SWE-agent (`max_requeries`). Tope largo por costo (USD 3-4) o iteraciones (100-500); SWE-agent auto-submitea el parcial al agotar.
- pass@1 Verified: OpenHands 53.0% (Sonnet 3.5) → 70.4% (Sonnet 4, 500 iter); SWE-agent 69% (Sonnet 4); Aider y Cursor no publican Verified. Único "con/sin loop": Aider Lite 20.3% → 26.3% con reintentos.
- Escalación a humano: por riesgo (OpenHands `ConfirmRisky`/`WAITING_FOR_CONFIRMATION`, Cursor Auto-review) o por límite agotado (Aider se detiene). Nadie escala por "no converge". Detalle y 3 ideas robables: `../research/09-fix-loops.md`.
