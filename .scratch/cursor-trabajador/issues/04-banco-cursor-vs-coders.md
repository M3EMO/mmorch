# Banco comparativo: Cursor contra el coder de DeepSeek y contra Claude
Type: prototype
Status: resolved
Blocked by: 01, 02, 03
Map: ../map.md

## Question

En las tareas de los bancos existentes, ¿cuánto acierta Cursor sin interfaz comparado con el coder de DeepSeek y con Claude, en cuánto tiempo, y cuánto cupo de Claude ahorra neto de la verificación? El diseño y el criterio se pre-registran en el ticket antes de correr.

## Answer

Resuelto (2026-10-01). 216 corridas válidas (3 errores del proveedor de Gemini apartados en `errores_api.jsonl` y repetidos), commits 31cd97e y el de resultados en `orch-spike/scripts/cursor_banco`.

| Brazo | Acopladas | Control | Segundos (mediana) | Diff (tokens, mediana) | Veredicto |
|---|---|---|---|---|---|
| Cursor `grok-4.7-medium` | 24/24 | 12/12 | 158 | 235 | Apto, recomendado por defecto |
| Cursor `gemini-3.8-flash-medium` | 24/24 | 12/12 | 402 | 222 | Apto, pero 2.5 veces más lento y fuera del pool |
| Cursor `gpt-5.4-mini-medium` | 22/24 | 12/12 | 217 | 315 | Apto (a 4 puntos de Claude) |
| Cursor `composer-2.5` | 14/24 | 12/12 | 139 | 171 | No apto (37 puntos abajo de Claude) |
| DeepSeek (agente del banco 09) | 23/24 | 12/12 | 43 | 224 | Sigue como coder de SDLC |
| Claude (`claude-sonnet-5`) | 23/24 | 12/12 | 116 | 228 | Línea base |

- Criterio "apto" (a 10 puntos o menos de Claude en acopladas, sin perder más de 10 en control): cumplen Grok, Gemini y GPT-mini; Composer no.
- Zona gris del coder de SDLC: DeepSeek queda a 4 puntos del mejor Cursor, así que sigue DeepSeek (43 s, US$0.01, paralelo y medido).
- Composer 2.5, el modelo del spike, era el peor: el 2/3 del spike no anticipaba el 24/24 de Grok. Falla sobre todo en t01, t06 y t07 (tareas acopladas con lector en otro módulo).
- Ahorro de cupo, estimado (sin criterio de corte, como se pre-registró): Claude implementando gasta por tarea, en mediana, 1.6 mil tokens de salida, 30 mil de cache nuevo y 376 mil de cache leído (US$0.22 equivalente con Sonnet 5). Verificar el trabajo de Cursor implica leer un pedido de ~300 tokens y un diff de ~235, más la respuesta de Claude: dos órdenes de magnitud menos tráfico. Con Opus en sesiones interactivas el ahorro es mayor. No se midió la verificación real.

## Notes

- Decidido en "Dónde entra Cursor" (2026-09-30): cuatro brazos de Cursor (`composer-2.5`, `grok-4.7-medium`, `gemini-3.8-flash-medium`, `gpt-5.4-mini-medium`) contra el coder de DeepSeek y contra Claude. Medir acierto, tiempo, tokens y el cupo de Claude que gasta la verificación leyendo el diff. Lanzar `cursor-agent` sin `SHELL` (hallazgo del spike).

**2026-09-30 — pre-registro (Claude, antes de correr):**

- Tareas: las 12 del banco del ticket 09 del mapa de impacto (8 acopladas, 4 de control; 30 señuelos por repo), con el informe de impacto instalado al inicio. Oráculo: el test oculto de cada tarea, ya validado.
- Brazos (3 repeticiones cada uno, 216 corridas):
  - Cursor ×4: `composer-2.5`, `grok-4.7-medium`, `gemini-3.8-flash-medium`, `gpt-5.4-mini-medium`. `cursor-agent -p --force --trust --output-format json`, sin `SHELL` en el entorno, pedido en `TAREA.md`.
  - DeepSeek: el agente del banco 09 con `deepseek-v4-pro` (15 pasos, mismas herramientas de siempre).
  - Claude: `claude -p --output-format json --permission-mode acceptEdits --allowedTools Bash`, modelo por defecto del plan, mismo `TAREA.md`. Gasta cupo de Claude (decisión del usuario).
- Cada corrida en una carpeta temporal con git. Medidas: acierto con el oráculo, tiempo, código de salida, archivos tocados, líneas y tokens aproximados del diff (lo que Claude leería al verificar), tokens del agente según su JSON.
- Criterio por modelo de Cursor: "apto como trabajador" si en acopladas queda a 10 puntos o menos de Claude y en control no pierde más de 10. Zona gris del coder de SDLC: si DeepSeek queda a 10 puntos o menos del mejor Cursor, sigue DeepSeek (más barato, paralelo y medido).
- Ahorro de cupo, estimado por tarea: tokens que gasta Claude implementando (su JSON) menos el costo de verificar (pedido + diff leído). Se informa la mediana por tarea, sin criterio de corte.
- Nota antes de la corrida completa: el brazo Claude corre con `claude-sonnet-5`, el modelo por defecto de `claude -p` en este plan. Prueba de humo en t05: Claude 65 s, costo equivalente US$0.22, diff de ~280 tokens; DeepSeek 29 s, US$0.008. Esas dos corridas cuentan como repetición 0 de t05.
