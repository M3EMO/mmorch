---
title: Motor de respuestas bot WhatsApp: intents pre-cargados + LLM con tools desde Java, modelo y costo
created: 2026-09-09
tags: [research, quetepario-chatbot, research, llm-pricing, spring-ai, tool-calling, java]
status: seed
sources: [https://platform.claude.com/docs/en/about-claude/pricing, https://api-docs.deepseek.com/zh-cn/quick_start/pricing, https://api-docs.deepseek.com/guides/tool_calls, https://devtk.ai/en/models/deepseek-v4-flash/, https://ai.google.dev/gemini-api/docs/pricing, https://ai.google.dev/gemini-api/docs/function-calling, https://docs.spring.io/spring-ai/reference/api/tools.html, https://docs.langchain4j.dev/tutorials/tools]
---
## Tronco

Un matcher de intents por tenant responde lo frecuente gratis; el LLM (Claude Haiku 4.5 vía Spring AI, tools estrictas, sin conocimiento libre) cubre el resto por menos de $3/mes en 300 conversaciones.

## Qué es

Ticket: `ChatBot/docs/agents/wayfinder/issues/03-motor-respuestas.md`. Fecha de research: 2026-09-09.
Pregunta: arquitectura mínima en Java para respuestas pre-cargadas + LLM, modelo barato con buen tool calling, SDK Java, garantía de "no inventa nada", costo mensual para 300 conversaciones bajo tope total de $15.

## Arquitectura mínima propuesta

Pipeline por mensaje entrante (webhook WhatsApp → Java):

1. **Normalizar.** Minúsculas, sin tildes, sin emojis. Detectar comandos duros (`hola`, `humano`, número de pedido).
2. **Matcher de intents por tenant.** Tabla `intent(tenant_id, patrones[], respuesta, umbral)`. Matching con keywords + regex + similitud de strings (Jaro-Winkler o trigramas; Apache Commons Text). Si score ≥ umbral, responde la respuesta pre-cargada. Costo: $0. Latencia: ms.
3. **LLM con tools solo si no matchea.** Una llamada a `ChatClient` con system prompt del tenant (tono, horarios, políticas, FAQ completo) y tools: `buscar_stock(talle, color)`, `buscar_faq(consulta)`, `armar_pedido(items)`, `cotizar_envio(cp)`, `derivar_humano(motivo)`.
4. **Regla de corte.** Dos turnos seguidos sin intent ni tool exitoso → `derivar_humano` y pausa del bot (igual que la demo del video: "tras 2 intentos sin entender, transfiere").

Embeddings como router: no para el MVP. Con ~30 intents por tenant, keywords + fuzzy alcanzan. Los embeddings agregan un proveedor más y una tabla vectorial. Reevaluar cuando el matcher falle >20 % en logs reales.

LLM como router: no. Cada mensaje costaría una llamada. El matcher local filtra el 80 % esperado.

## Técnica "no inventa nada" (grounding)

Cuatro capas, todas en Java, ninguna depende de que el LLM "se porte bien":

1. **Datos solo vía tools y system prompt.** El prompt dice: "Respondé solo con datos del bloque TENANT o de resultados de tools. Si no hay dato, llamá `derivar_humano`." El LLM no recibe instrucción de usar conocimiento propio.
2. **Tools estrictas.** Claude: `strict: true` en la tool (schema validado, GA sin beta). DeepSeek: `strict: true` vía `base_url .../beta`. Gemini: modo `validated`. Así `armar_pedido` nunca recibe un talle inventado fuera del enum del catálogo.
3. **Structured output con `source_ids`.** La respuesta final es JSON `{texto, source_ids[]}`. Java rechaza la respuesta si `source_ids` está vacío y no hubo tool call. En ese caso responde el mensaje de transferencia del tenant. Claude: `output_config.format`. Spring AI: `.entity(Clase.class)`.
4. **Tool forzada en pedidos.** Para intents de compra, `tool_choice: any` obliga a pasar por `armar_pedido` (Haiku 4.5 lo soporta; Claude Fable 5.1 no, pero no aplica acá).

Prompt caching: el bloque TENANT (FAQ, políticas, catálogo resumido) va primero con `cache_control`. En Claude el cache read cuesta 0.1× del input. Mínimo cacheable depende del modelo (512–4096 tokens).

## Tabla de modelos (precio por 1M tokens, USD, verificado 2026-09-09)

Supuesto de conversación: 10 turnos, **~4k tokens input totales y ~1k output totales** (el supuesto del ticket; ver objeción abajo). Costo = 4k×in + 1k×out.

| Modelo | Input | Output | $/conv (4k/1k) | $/mes 300 conv | Fuente |
|---|---|---|---|---|---|
| Claude Haiku 4.5 | $1.00 | $5.00 | $0.0090 | $2.70 | platform.claude.com/docs/en/about-claude/pricing |
| Claude Sonnet 5 | $2.00 | $10.00 | $0.0180 | $5.40 | ídem |
| Gemini 2.5 Flash-Lite | $0.10 | $0.40 | $0.0008 | $0.24 | ai.google.dev/gemini-api/docs/pricing |
| Gemini 2.5 Flash | $0.30 | $2.50 | $0.0037 | $1.11 | ídem |
| Gemini 3.5 Flash-Lite | $0.30 | $2.50 | $0.0037 | $1.11 | ídem |
| Gemini 3.8 Flash | $0.75 (hasta 2026-12-31; $1.50 desde 2027) | $3.75 ($7.50 desde 2027) | $0.0068 | $2.03 | ídem |
| DeepSeek v4-flash peak | $0.44 | $1.32 | $0.0031 | $0.92 | api-docs.deepseek.com (zh-cn: 3.0元 / 9.0元) + devtk.ai |
| DeepSeek v4-flash off-peak | $0.22 | $0.66 | $0.0015 | $0.46 | ídem (1.5元 / 4.5元) |
| DeepSeek v4-pro peak | ≈$1.32 | ≈$3.96 | $0.0092 | $2.77 | oficial solo en CNY: 9.0元 / 27.0元; USD derivado, no verificado |

Notas de precio:
- Claude cache read: Haiku 4.5 $0.10/M, Sonnet 5 $0.20/M. Batch API no aplica (chat en vivo).
- Claude tool use agrega ~496 tokens de system prompt por llamada en Haiku 4.5 (tabla oficial de pricing).
- DeepSeek: la página oficial en inglés no renderiza por fetch. Los USD salen de la página oficial en CNY más devtk.ai (que cita la oficial, verificado 2026-08-17). Cache hit v4-flash: $0.014 peak / $0.007 off-peak (coincide con `~/.claude/orchestration/prices.json`, verificado 2026-09-04).
- DeepSeek peak = 01:00–04:00 y 06:00–10:00 UTC, lunes a viernes. En Argentina (UTC-3) eso es 22:00–01:00 y 03:00–07:00. El horario comercial argentino cae casi todo en off-peak.
- Gemini 3.x Flash sube de precio el 2027-01-01. Gemini 2.5 Flash-Lite es el más barato de la tabla.

**Dato que NO se encontró:** ningún benchmark primario de tool calling en español para estos modelos. La recomendación de modelo se basa en soporte de schema estricto y precio, no en una medición de español. Medir con 30 conversaciones reales de QTP antes de fijar.

## SDK Java recomendado: Spring AI 2.0.1

Justificación:
- El stack decidido es Spring Boot. Spring AI es el módulo nativo: `ChatClient`, `@Tool` sobre métodos, `ToolCallingAdvisor` que corre el loop de tools solo.
- Soporta Anthropic, Google GenAI y DeepSeek como `ChatModel`. Cambiar de modelo primario a fallback es cambiar un starter y una propiedad. Eso habilita el fallback cross-family sin tocar código de negocio.
- Advisors componen memoria de conversación, observabilidad y retries en la misma cadena.

Alternativas descartadas:
- **LangChain4j** (`@Tool` + `AiServices`): funciona, pero es una abstracción paralela a Spring dentro de un proyecto Spring. Dos formas de hacer lo mismo suben la carga del próximo lector.
- **Anthropic Java SDK 2.34.0 directo** (`com.anthropic:anthropic-java`, `BetaToolRunner`): la opción correcta si se fija un solo proveedor. Pierde el swap de fallback por configuración.
- **HTTP directo**: reimplementa el loop de tools, parsing de `tool_use`, retries y streaming. No.

## Modelo recomendado y fallback

- **Primario: Claude Haiku 4.5** (`claude-haiku-4-5`). Tools estrictas GA, `tool_choice: any`, structured output, prompt caching. $2.70/mes en el peor caso (todas las conversaciones al LLM). Cabe en el tope de $15 con margen para WhatsApp.
- **Fallback: Gemini 2.5 Flash** (familia distinta, $1.11/mes). Spring AI Google GenAI starter. Modo `validated` para schema.
- **Opción ultra barata: DeepSeek v4-flash** ($0.46–$0.92/mes). Strict tools en beta. Riesgo: mmorch midió que el mismo modelo con thinking apagado refuta ~40 % del trabajo correcto como verificador (n=350). Eso es otro rol, no generación de chat, pero es una señal de fragilidad sin thinking. Si se usa, dejar thinking prendido y medir.

## Costo mensual para 300 conversaciones

Escenario A (supuesto del ticket, 4k in / 1k out por conversación, 100 % al LLM): Haiku 4.5 = **$2.70**. Con matcher que resuelva 80 % de las conversaciones: **~$0.54**.

Escenario B (realista sin cache): cada turno reenvía historial + system prompt de ~3k tokens. 10 turnos ≈ 40k input, 1k output. Haiku 4.5 = $0.045/conv = **$13.50/mes**. Esto sí rompe el tope de $15.

Escenario B con prompt caching (bloque TENANT cacheado, ~90 % de input como cache read a $0.10/M): ≈ $0.004 + $0.0004×9 + $0.005 ≈ $0.013/conv = **~$3.80/mes**. Con matcher al 80 %: **~$0.76**.

Conclusión: el presupuesto LLM queda entre $0.50 y $4/mes si se hacen dos cosas: matcher local primero y prompt caching del bloque del tenant. Sin esas dos, Haiku puede consumir todo el tope.

## Aplicable a mmorch

mmorch **no sirve como capa de ruteo en runtime** para el bot. Fundamento:
- Es una librería Python + MCP server pensado para Claude Code. No hay cliente Java ni API HTTP estable de ruteo; `mmorch.server` es un dashboard de jobs, no un gateway de inferencia.
- `mmorch_route` hace ruteo por **auto-confianza del LLM** (self-score) para escalar a Opus y ahorrar cupo. El bot necesita ruteo por **datos del tenant** (intent matcheó o no), que es determinista y vive en Java.
- Meter un salto de proceso Python por mensaje de WhatsApp agrega latencia y un punto de falla sin beneficio: el fallback cross-family ya lo da Spring AI por configuración.

Sí sirve **offline** como banco de medición:
- `fan_out` para generar variantes de FAQ y mensajes de tenant en bulk.
- `adversarial_verify` (checkeable → `checkers.py`) para validar que respuestas pre-cargadas coincidan con el catálogo.
- Armar un gold set de 50 mensajes reales → intent esperado, y medir el matcher y cada modelo con etiqueta computable. Eso responde el dato que falta (calidad en español).

## Objeciones

1. El supuesto "4k tokens por conversación de 10 turnos" es optimista. Cada turno reenvía el historial. Ver escenario B. El dato que falta es el tamaño real del system prompt de QTP.
2. No hay medición de tool calling en español. Toda la tabla es precio, no calidad. Requiere el gold set del punto anterior.
3. El precio USD de DeepSeek v4-pro está derivado de CNY. No citar como oficial.
4. Gemini 3.x cambia precio en 2027. Si se elige Gemini, fijar 2.5 Flash o revisar en diciembre.

## Veredicto cross-family

- Pendiente. Nota en estado `seed`. Correr `/verify-cross` sobre la tabla de costos antes de fijar presupuesto.

## Links

- Ticket: `QueTePario/ChatBot/docs/agents/wayfinder/issues/03-motor-respuestas.md`
- Mapa: `QueTePario/ChatBot/docs/agents/wayfinder/map.md`
- Registro de precios mmorch: `~/.claude/orchestration/prices.json`, `mmorch/config.py`
