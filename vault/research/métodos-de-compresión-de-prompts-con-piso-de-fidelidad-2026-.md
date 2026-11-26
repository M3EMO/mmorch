---
title: Métodos de compresión de prompts con piso de fidelidad (2026-11-26)
created: 2026-11-26
tags: [research, orchestration, vault-y-compresor, compresion]
status: seed
confidence: "media: todas las cifras vienen de abstracts o resúmenes de página; ninguna se leyó en el cuerpo del paper ni se re-corrió"
sources: ["https://arxiv.org/abs/2403.12968", "https://arxiv.org/abs/2310.06201", "https://arxiv.org/abs/2310.06839", "https://arxiv.org/abs/2603.29919", "https://arxiv.org/abs/2505.00019", "https://arxiv.org/abs/2606.24083", "https://arxiv.org/abs/2606.03618", "https://arxiv.org/abs/2607.25335", "https://lacuna.tiptreesystems.com/direction/coherence-preserving-context-compression-for-language-models-7006", "https://lacuna.tiptreesystems.com/direction/the-compression-fidelity-tradeoff-in-prompt-compression-6898"]
---
# Métodos de compresión de prompts con piso de fidelidad

Complementa la nota "Vault y compresor: babel no paga, el arranque sí (2026-11-26)". No repite StaleBench, Memory Tax, rate-distortion, Beyond Token Savings, Context Has a Price ni la cifra base de SkillReducer.

Texto objetivo: CLAUDE.md, descripciones de skills y notas markdown en español. El lector es Claude.

## Tabla por método

| Método | Ahorro medido | Pérdida medida | Dónde corre | Query-aware | Salida legible |
|---|---|---|---|---|---|
| LLMLingua-2 (2403.12968) | 2x-5x en tokens; compresor 3x-6x más rápido; latencia total 1.6x-2.9x menor | El abstract no da pérdida. Sobre MeetingBank, LongBench, ZeroScrolls, GSM8K y BBH | Local: encoder XLM-RoBERTa-large o mBERT, corre en CPU lento o en GPU chica | No, es task-agnostic | No: borra tokens sueltos, el texto queda fragmentado |
| Selective Context (2310.06201) | Costo de contexto -50%, memoria -36%, tiempo -32% | BERTscore -0.023, faithfulness -0.038 en resumen, QA y respuesta | Local: LM chico para surprisal | No | Parcial: borra frases o palabras, sin tokens sueltos |
| LongLLMLingua (2310.06839) | 2x-6x en prompts de ~10k; costo -94% en LooGLE; latencia 1.4x-2.6x menor | Accuracy +21.4% en NaturalQuestions con GPT-3.5 (mejora por reordenar y filtrar) | Local con un LM tipo Llama | Sí | No |
| SkillReducer (2603.29919) | Descripciones -48%, cuerpos -39% | Calidad funcional +2.8%; retención media 0.965 en 5 modelos de 4 familias; 600 skills y SkillsBench | Usa LLM por API (inferido, el abstract no lo dice) | No para el ahorro; el cuerpo se parte en reglas núcleo y contenido bajo demanda | Sí: prosa normal |
| Borrado de tramos / sentencias | FROST: -69.68% de tokens en trazas de razonamiento (Lacuna, sin benchmark en la página) | FROST +26.70% de accuracy; el borrado de tokens sueltos sube las alucinaciones | Local, según el método | Según el método (QUITO sí) | Sí si la unidad es la oración; no si es el token |
| Reescritura por LLM (KiS, resumen) | KiS no tiene cifra en el estudio 2505.00019 | El estudio no la evalúa. Los resúmenes automáticos pierden restricciones de formato (Lacuna, Less is More) | API o LLM local | Posible | Sí |
| Reglas lingüísticas, EMNLP 2026 (2607.25335) | Sin cifra en el abstract | "Similar" a métodos recientes con compresión leve o moderada; peor con compresión fuerte | CPU, sin pasada de LM | No | No verificado |
| Reescritura local con Llama 3.2 3B (2606.03618) | Tokens del prompt -34% a -47%; total hasta -18.8% | "Preserva o mejora" accuracy; sin cifra. Supera a LLMLingua-2 en OckScore | Local, LM de 3B | No | Sí, pero traduce a inglés y reestructura |

## Hallazgos que cambian la recomendación

- Estudio de seis métodos en 13 datasets (2505.00019, ICLR workshop 2025): LLMLingua, LongLLMLingua y LLMLingua-2 aguantan mejor a ratios altos.
- En ese estudio todo método sube las alucinaciones. LongLLMLingua tuvo MaHR 0.20 y LLMLingua-2 0.21; la selección al azar tuvo 0.70.
- En contexto corto la calidad baja al subir el ratio. En contexto largo una compresión moderada puede mejorar el resultado.
- CAVEWOMAN (2606.24083, 8 modelos, 5 datasets): comprimir la entrada en estilo telegráfico sube el costo neto unos 1.15x de media, hasta 1.8x en el peor dataset y hasta 2.7x con compresión fuerte.
- En CAVEWOMAN el modelo compensa con respuestas más largas. Comprimir la salida del modelo sí ahorra, de 1.4x a 2.4x.
- Un estudio de latencia (2604.02985) mide hasta 18% de aceleración con modelos propios, sin cambio estadístico de calidad.
- Un caso de código (2606.03618): LLMLingua-2 bajó la accuracy de 99.50% a 77.33% con GPT-3.5, y casi no la movió con GPT-4o (98.33% a 97.17%).
- Lacuna (dirección "Compression Fidelity Tradeoff"): la compresión fuerte quita datos aislados, como un nombre propio o un número. Ese dato el modelo no lo recupera.
- Lacuna: los resúmenes automáticos de instrucciones largas pierden restricciones de formato.

## Qué aplica a este caso

- LLMLingua-2 y Selective Context dejan texto ilegible o fragmentado. Los CLAUDE.md y las skills los edita una persona, así que no sirven como archivo fuente.
- El texto objetivo es instrucción corta, no contexto largo. En contexto corto la compresión da menos y pierde más.
- Un método query-aware necesita la pregunta. En el arranque no hay pregunta, así que solo el compresor sin pregunta aplica al arranque.
- Con prompt caching, cualquier cambio en el prefijo cacheado invalida el caché. Un ahorro de 20% en tokens puede costar más en la primera escritura.
- SkillReducer es el único método con piso medido (retención 0.965) sobre el tipo de texto de este caso, con salida legible.

## Lagunas

- Ninguna cifra viene de un español. LLMLingua-2 usa encoders multilingües, pero se entrenó con MeetingBank en inglés. No hay medición propia.
- Ninguna cifra se leyó en el cuerpo del paper, salvo las de la tabla 5 de 2505.00019 que devolvió la página.
- No hay una medición con el contador real de Anthropic sobre notas en español. Hay que medirla antes de elegir.
- No encontré un método que mida legibilidad humana de forma directa.
