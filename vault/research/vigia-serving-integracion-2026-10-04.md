---
title: Vigia - integracion de senales del router MoE en servidores de inferencia
mision: Como extrae Vigia entropia del router y desacuerdo entre expertos en HF transformers, vLLM, SGLang, llama.cpp, TGI y Ollama, y como lo expone por una API compatible con OpenAI sin romper clientes.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
  - https://docs.vllm.ai/en/latest/api/vllm/model_executor/layers/fused_moe/routed_experts_capturer/
  - https://docs.vllm.ai/en/latest/examples/rl/routed_experts_e2e/
  - https://docs.vllm.ai/en/v0.14.0/api/vllm/model_executor/layers/fused_moe/fused_moe_router/
  - https://docs.vllm.ai/en/latest/design/plugin_system.html
  - https://docs.vllm.ai/en/latest/cli/bench/serve/
  - https://docs.vllm.ai/en/latest/getting_started/installation/
  - https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
  - https://discuss.vllm.ai/t/implementing-hidden-state-probes/2291
  - https://github.com/vllm-project/vllm/issues/19342
  - https://vllm.ai/blog/2026-03-30-extract-hidden-states
  - https://arxiv.org/html/2603.06588
  - https://github.com/IBM/vllm-hook
  - https://raw.githubusercontent.com/sgl-project/sglang/main/python/sglang/srt/eplb/expert_distribution.py
  - https://raw.githubusercontent.com/sgl-project/sglang/main/python/sglang/srt/managers/io_struct.py
  - https://docs.sglang.io/basic_usage/native_api.html
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/master/ggml/include/ggml-backend.h
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/master/src/llama-graph.cpp
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/master/src/llama-context.cpp
  - https://raw.githubusercontent.com/ggml-org/llama.cpp/master/examples/eval-callback/eval-callback.cpp
  - https://huggingface.co/docs/text-generation-inference/en/index.md
  - https://github.com/ollama/ollama/releases/tag/v0.12.11
  - https://huggingface.co/docs/transformers/main/en/model_doc/gpt_oss
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/qwen3_moe/modeling_qwen3_moe.py
  - https://huggingface.co/openai/gpt-oss-20b/raw/main/config.json
  - https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct
  - https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM/raw/main/config.json
  - https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
  - https://huggingface.co/docs/transformers/main/en/generation_strategies
  - https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/chat/chat_completion_token_logprob.py
  - https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/chat/chat_completion_chunk.py
  - https://raw.githubusercontent.com/openai/openai-python/main/src/openai/_models.py
  - https://raw.githubusercontent.com/openai/openai-python/main/README.md
  - https://docs.pytorch.org/tutorials/recipes/recipes/benchmark.html
  - https://arxiv.org/html/2608.17687
created: 2026-10-04
---

# Vigia - integracion de senales del router en serving de produccion

## Tronco

Vigia arranca con un backend HF transformers con hooks sobre el router (barato y portable), define un contrato `SignalBackend` que emite features escalares por token, y llega a vLLM con un plugin que calcula esas features en GPU dentro del `custom_routing_function`, copiando el patron de `RoutedExpertsCapturer` (~2% de throughput medido por vLLM para solo IDs).

## Hallazgos

### H1. Que necesita Vigia del modelo (define todo lo demas)

El paper InnerExpert (arXiv 2608.17687) extrae seis senales MoE por token, en cada capa:

| Senal | Insumo minimo | Disponible solo con IDs de expertos |
|---|---|---|
| Entropia del router (Shannon sobre la distribucion de gating) | logits completos del router `[tokens, n_experts]` | No |
| Expert hidden score (ponderado por routing) | salidas por experto + pesos top-k | No |
| Similitud entre expertos (coseno par a par ponderado; mide desacuerdo) | salidas por experto ANTES de la suma ponderada | No |
| Distribucion acumulada de uso de expertos | IDs + pesos top-k | Parcial (sin pesos) |
| Impureza de Gini del uso | idem | Parcial |
| Indice de Herfindahl inverso (numero efectivo de expertos) | idem | Parcial |

- El paper tambien usa senales estandar: score de hidden states y score de atencion.
- El paper prueba OLMoE-1B-7B (16 capas, 64 expertos) y Gemma-4-26B (30 capas, 128+1 expertos).
- El paper reporta costo "~3x vanilla overall" y ~3% de memoria pico extra.
- Ese 3x choca de frente con la meta del plan (< 5% de latencia agregada).
- Consecuencia: Vigia necesita niveles de senal con costo distinto. La seccion de implicancias lo baja a requisitos.

### H2. Forma del router en los modelos objetivo (HF transformers, rama main)

| Modelo | Capas | Expertos | Top-k | Que devuelve el router |
|---|---|---|---|---|
| gpt-oss-20b | 24 | 32 | 4 | `(router_logits, router_scores, router_indices)`; softmax SOLO sobre los top-k valores |
| gpt-oss-120b (defaults de `GptOssConfig`) | 36 | 128 | 4 | idem |
| Qwen3-MoE | config | config | config | `(router_logits, router_scores, router_indices)`; `norm_topk_prob` renormaliza top-k |
| Granite 3.1 1B-A400M | 24 | 32 | 8 | `GraniteMoeForCausalLM` [forma exacta del router no verificada] |
| tiny-random-Mixtral (hf-internal-testing) | 2 | 4 | 2 | Mixtral [no verificado en detalle] |

- gpt-oss calcula `router_logits = F.linear(hidden_states, weight, bias)` y luego `topk` y `softmax` sobre los top-k.
- Por eso la entropia "del modelo" (sobre top-k) no es la entropia completa. Vigia debe calcular su propio softmax sobre los `n_experts` logits.
- Transformers v5 registra router logits con `_can_record_outputs = {"router_logits": OutputRecorder(<Router>, index=0)}`.
- El docstring de salida dice "Raw router logits (post-softmax)". Esto es ambiguo entre modelos. Vigia debe detectar si recibe logits o probabilidades (filas que suman 1).
- El cuantizado MXFP4 de gpt-oss-20b excluye `model.layers.*.mlp.router` de la conversion. El router queda en precision completa. Esto es bueno para la estabilidad de la entropia.
- gpt-oss no soporta SDPA (necesita attention sinks). En CPU eso obliga a eager o flex attention [impacto en CPU no verificado].
- `GptOssMLP.forward` llama `self.experts(hidden_states, router_indices, router_scores)`. El modulo de expertos devuelve la suma ya ponderada. Un hook de forward no ve salidas por experto.

### H3. HF transformers como backend del MVP

- Ruta A: `output_router_logits=True` en `generate()`. Falla con cache KV: el foro de HF (nov 2025) reporta un `RuntimeError` por mismatch de tamanos en la aux loss. El flag existe para entrenamiento.
- Ruta B (recomendada): `register_forward_hook` sobre cada modulo router. Con cache KV, cada paso de decode entrega solo el token nuevo. El hook ve `[batch, 1, n_experts]` por capa.
- Ruta C: loop de decodificacion propio via `custom_generate` callable. `generate()` acepta un callable que reemplaza solo el loop y conserva la preparacion de inputs.
- Para similitud entre expertos hay dos opciones: monkeypatch del `forward` del modulo de expertos para guardar salidas por experto, o recomputar los k expertos. Recomputar duplica el costo del FFN MoE.
- Streaming en HF: `TextIteratorStreamer` existe [no verificado en esta sesion; conocido de versiones previas].

### H4. vLLM (arquitectura v1)

| Mecanismo | Que da | Logits del router | Salidas por experto | Streaming | Costo medido |
|---|---|---|---|---|---|
| `--enable-return-routed-experts` (desde v0.14) | IDs top-k: `routed_experts [gen_len, n_moe_layers, top_k]` int16 y `prompt_routed_experts` | No | No | No: llega al terminar el request | ~2% throughput; ~14 MB/GPU (40 capas, 8192 tokens, top-22) |
| Extraccion de hidden states (PR #33736, v0.18) | hidden states de capas elegidas via KV connector a safetensors | No | No | No; solo prompt (`max_tokens=1`) | "sin overhead en hot path" segun blog |
| vLLM Hook v0 (IBM, arXiv 2603.06588) | forward hooks en un worker subclase; guarda a disco con `torch.save` | Posible via hook | Posible via hook | No | No reportado |
| Plugin propio (`vllm.general_plugins` + `ModelRegistry.register_model`) | lo que Vigia calcule | Si | Con patch de kernel | Si, si se agrega endpoint | A medir |

Detalles verificados:

- El issue #19342 ("How to get the router logits for an MoE model?") se cerro como stale, sin solucion.
- El foro de vLLM (oct 2025) confirma: no hay soporte nativo para router logits.
- El foro de vLLM (ene 2026) sobre probes de hidden states: sigue fuera de core por "performance, complexity, and multi-tenant stability".
- `RoutedExpertsCapturer` vive en `vllm/model_executor/layers/fused_moe/routed_experts_capturer.py`.
- El capturer recibe `capture(layer_id, topk_ids)` desde hooks de capa dentro del forward.
- El capturer escribe en un buffer de dispositivo preasignado `(max_num_batched_tokens, num_layers, num_experts_per_tok)` int32.
- Al final del paso, `GPUModelRunner` copia D2H a un buffer pinned de CPU.
- Ese patron es compatible con CUDA graphs: escrituras in-place en buffers fijos.
- Limitaciones del routed experts replay: no sirve con requests preemptados, exige scheduling sincrono, no soporta sequence parallelism, pipeline parallelism ni prefill-context parallelism. Solo motor V1.
- La API OpenAI expone `prompt_routed_experts` (nivel response) y `routed_experts` (nivel choice) en `/v1/completions`. Chat completions no aparece documentado.
- `FusedMoE` acepta `custom_routing_function(hidden_states, gating_output, topk, renormalize) -> (topk_weights, topk_ids)`. `gating_output` son los logits del router. Es el punto de insercion mas limpio para calcular entropia en GPU.
- La abstraccion nueva es `FusedMoERouter.select_experts(hidden_states, router_logits) -> (topk_weights, topk_ids)`.
- El sistema de plugins tiene cinco grupos: `vllm.general_plugins`, `vllm.platform_plugins`, `vllm.io_processor_plugins`, `vllm.stat_logger_plugins`, `vllm.endpoint_plugins`. Los endpoint plugins no cargan por default.
- Cada proceso de vLLM carga los plugins. El plugin debe ser re-entrante.
- vLLM no lista Windows como SO soportado. CPU soportado: x86, AArch64, Apple silicon, S390X.

### H5. SGLang

- Endpoints nativos: `/start_expert_distribution_record`, `/stop_expert_distribution_record`, `/dump_expert_distribution_record`.
- Flag del server: `--expert-distribution-recorder-mode` con modos `stat`, `stat_approx`, `per_pass`, `per_token`.
- `per_token` guarda `topk_ids_of_layer` con forma `(num_layers, num_tokens, TOP_K_NUM)` mas `input_ids`, `positions`, `extend_seq_lens`, `forward_mode`.
- El dump va a archivos `.pt` en `SGLANG_EXPERT_DISTRIBUTION_RECORDER_DIR`. Es global del server, no por request.
- Hook interno: `on_select_experts(topk_ids)`. Solo IDs, no logits.
- `GenerateReqInput` tiene `return_routed_experts: bool = False` y `routed_experts_start_len`. Es por request. [flag de server asociado no verificado: no estaba en `server_args.py`].
- `GenerateReqInput` tambien tiene `return_hidden_states`, `return_logprob`, `top_logprobs_num`, `custom_logit_processor`.
- Conclusion: SGLang da IDs por request, igual que vLLM. Para entropia hace falta patch del router.

### H6. llama.cpp

- Callback de evaluacion: `typedef bool (*ggml_backend_sched_eval_callback)(struct ggml_tensor * t, bool ask, void * user_data);`.
- Con `ask == true` el scheduler pregunta si el usuario quiere observar el nodo. Esto permite agrupar nodos.
- Con `ask == false` el scheduler entrega el tensor. Si el callback devuelve false, cancela el compute.
- Se registra con `params.cb_eval` y `params.cb_eval_user_data` (contexto).
- Los tensores se nombran `"%s-%d"` con el indice de capa (por ejemplo `ffn_moe_logits-12`).
- `build_moe_ffn` nombra: `ffn_moe_logits`, `ffn_moe_probs`, `ffn_moe_topk`, `ffn_moe_weights`, `ffn_moe_weights_norm`.
- `ffn_moe_down` tiene forma `[n_embd, n_expert_used, n_tokens]`: son las salidas por experto ANTES de `ffn_moe_weighted` y `ffn_moe_out`.
- Entonces llama.cpp expone TODO lo que Vigia necesita: logits completos y salidas por experto.
- Funciones de gating soportadas: softmax, sigmoid, softmax_weight, sqrt_softplus.
- `llama-server` no expone el callback por HTTP. Hace falta un binario propio o bindings. [cb_eval en llama-cpp-python: no verificado].
- Observar nodos fuerza sincronizacion y copia desde el backend. El costo en GPU no esta medido [no verificado].
- Ventaja para el desarrollo: corre en CPU Windows con GGUF de modelos MoE chicos (Granite 3.1 1B-A400M tiene cuantizaciones listadas en HF).

### H7. TGI y Ollama

- TGI esta en modo mantenimiento desde diciembre de 2025. HF recomienda vLLM, SGLang, llama.cpp o MLX. Vigia no debe invertir en TGI.
- Ollama soporta `logprobs` y `top_logprobs` desde v0.12.11 (12 nov 2025), en API nativa y OpenAI-compatible.
- Ollama no expone tensores internos ni router [no verificado: no existe API documentada]. Integrar requiere fork.
- Ollama sirve como fuente de baseline barato: entropia de logprobs del vocabulario, sin router.

### H8. Matriz de viabilidad por servidor

| Servidor | Entropia router | Desacuerdo expertos | Por request | Streaming | Esfuerzo | Latencia esperada | Veredicto |
|---|---|---|---|---|---|---|---|
| HF transformers + hooks | Si (hook router) | Si (patch de experts) | Si | Si (propio) | Bajo | Bajo para router; alto si recomputa expertos | MVP |
| vLLM nativo (`routed_experts`) | No | No | Si | No | Nulo | ~2% medido | Solo senales de uso (Gini, HHI sin pesos) |
| vLLM + plugin Vigia | Si (en `custom_routing_function`) | Requiere patch de kernel fused | Si | Si con endpoint propio | Alto | A medir; objetivo < 5% | Ruta de produccion |
| SGLang nativo | No | No | Si (`return_routed_experts`) | [no verificado] | Nulo | [no medido] | Igual que vLLM nativo |
| SGLang + patch | Si | Patch | Si | Si | Alto | A medir | Segunda ruta |
| llama.cpp + `cb_eval` | Si (`ffn_moe_logits`) | Si (`ffn_moe_down`) | Si | Si | Medio (C++) | [no medido] | Ruta edge/CPU |
| TGI | - | - | - | - | - | - | Descartado (mantenimiento) |
| Ollama | No | No | - | - | Fork | - | Solo baseline con logprobs |

### H9. API OpenAI-compatible y extension compatible

- `ChatCompletionTokenLogprob` tiene `token`, `bytes`, `logprob`, `top_logprobs`. El valor `-9999.0` marca tokens fuera del top-20.
- En streaming, cada `chat.completion.chunk` trae `choices[].delta`, `finish_reason`, `index`, `logprobs` (con `content` y `refusal`).
- Con `stream_options: {"include_usage": true}` el ultimo chunk trae `usage` y puede traer `choices` vacio.
- El SDK Python de OpenAI usa pydantic con `extra="allow"`. Los campos desconocidos sobreviven y se leen como atributo o con `model_extra`.
- El SDK envia parametros no documentados con `extra_body`, `extra_query`, `extra_headers`.
- vLLM ya usa este patron: agrega `routed_experts` y `prompt_routed_experts` como campos extra.
- Conclusion: un campo extra por chunk no rompe al SDK oficial. Un campo extra dentro de `logprobs.content[]` tambien sobrevive por `extra="allow"` [no verificado contra SDKs de otros lenguajes].

### H10. Medicion justa de overhead

- vLLM trae `vllm bench serve` con TTFT, TPOT, ITL y E2EL.
- Flags relevantes: `--percentile-metrics` (default `ttft,tpot,itl`), `--metric-percentiles` (default `99`), `--num-warmups` (default 0), `--request-rate`, `--max-concurrency`, `--burstiness`, `--dataset-name random`, `--random-input-len` (1024), `--random-output-len` (128), `--ignore-eos`, `--seed`, `--save-result`.
- El default `--num-warmups 0` es una trampa. Hay que fijarlo explicitamente.
- `torch.utils.benchmark.Timer` hace warmup, sincroniza CUDA y fija un thread por default. `blocked_autorange` acumula al menos 0.2 s de mediciones.
- `--ignore-eos` fija la longitud de salida. Sin eso, Vigia podria cambiar la longitud y sesgar el tok/s.

## Implicancias para Vigia

### Arquitectura

1. Definir el contrato `SignalBackend` (Python, `Protocol`) con estos metodos:
   - `capabilities() -> BackendCaps` (flags: `router_logits`, `expert_outputs`, `routed_ids_only`, `streaming`, `max_batch`).
   - `model_info() -> ModelInfo` (familia, `n_layers`, `n_experts`, `top_k`, `gating_fn`, `router_output_kind` = logits o probs).
   - `generate(request) -> Iterator[TokenEvent]` donde cada `TokenEvent` trae `token_id`, `text`, `logprob`, `features: dict[str, float]`, `layer_features: array[n_layers, F] | None`.
2. El contrato emite FEATURES escalares, no logits crudos. Un vector `[n_layers, F]` en float16 cuesta mucho menos que `[n_layers, n_experts]`.
3. El detector (puntaje por token) consume solo `TokenEvent.features`. El detector no conoce el backend.
4. Implementar el calculo de features en un modulo puro `vigia.features` (torch, sin estado). HF, vLLM y los tests lo comparten.
5. Backend 1 (MVP): `HFTransformersBackend` con `register_forward_hook` sobre la clase router de cada familia.
6. Backend 2 (tests y CI): `TinyMoEBackend` sobre `hf-internal-testing/tiny-random-MixtralForCausalLM` (2 capas, 4 expertos, top-2). Corre en CPU en segundos.
7. Backend 3 (dev realista en CPU): Granite 3.1 1B-A400M (Apache 2.0, soporta espanol, 32 expertos, top-8) en HF transformers.
8. Backend 4 (produccion): `VllmPluginBackend` registrado via `vllm.general_plugins`. El plugin envuelve `custom_routing_function` / `select_experts`, calcula features en GPU y escribe en un buffer preasignado como `RoutedExpertsCapturer`.
9. Backend 5 (edge, opcional): `LlamaCppBackend` en C++ con `cb_eval` filtrando `ffn_moe_logits-*` y `ffn_moe_down-*` en la fase `ask`.
10. Backend 6 (degradado): `RoutedIdsBackend` que consume `routed_experts` de vLLM/SGLang sin patch. Solo da senales de uso. Debe declararlo en `capabilities()`.

### Niveles de senal (para cumplir < 5%)

11. Nivel R (router-only): entropia completa, margen top1-top2, masa top-k, Gini y HHI inverso con pesos. Costo: un softmax `[tokens, n_experts]` por capa. Default en produccion.
12. Nivel E (expertos): similitud coseno ponderada entre salidas de expertos. Requiere salidas por experto. Opt-in, con advertencia de costo.
13. Nivel U (uso): Gini/HHI sin pesos desde IDs. Disponible en vLLM/SGLang sin patch.
14. Medir AUROC por nivel. Si R logra AUROC cercano a R+E, R es el producto. Si no, E se vuelve feature paga para lotes offline.
15. El reporte debe mostrar AUROC y overhead lado a lado por nivel.

### Hooks HF (requisitos concretos)

16. Registrar hooks por clase (`GptOssTopKRouter`, `Qwen3MoeTopKRouter`, gate de Mixtral, Granite, OLMoE) en un registro `ROUTER_ADAPTERS` por familia.
17. Cada adaptador declara si su salida son logits o probabilidades. Un test valida filas que suman 1.
18. Calcular la entropia sobre `n_experts` completos, no sobre los top-k que usa gpt-oss.
19. No usar `output_router_logits=True` en `generate()` con cache.
20. En decode, el hook ve un token por paso. En prefill, el hook ve todo el prompt. Separar features de prompt y de respuesta.
21. Hacer `.detach()` y reducir a features dentro del hook. No acumular tensores de `n_experts` por token en memoria.
22. Envolver todo en `torch.inference_mode()`.

### API

23. Exponer `/v1/chat/completions` y `/v1/completions` compatibles con el SDK oficial de OpenAI.
24. Activar Vigia por request con `extra_body={"vigia": {"enabled": true, "level": "R", "threshold": 0.5}}`. Sin ese campo, la respuesta es identica a OpenAI.
25. Respuesta no streaming: agregar `choices[i].vigia = {"tokens": [{"index", "confidence", "flag"}], "spans": [...], "model_version", "calibration_id"}`.
26. Respuesta streaming: agregar `choices[i].vigia` en cada chunk con los tokens del `delta`. Mantener `logprobs` estandar intacto.
27. Emitir los `spans` (fragmentos de baja confianza) agregados en el chunk final, antes de `data: [DONE]`.
28. Alinear confianza a tokens del tokenizer del modelo, igual que `logprobs.content[]`. Incluir `text_offset` en caracteres para que el panel resalte sin re-tokenizar.
29. Documentar ejemplo con `openai` Python: leer `chunk.choices[0].model_extra["vigia"]`.
30. Exponer la accion configurada por fragmento (`review`, `rag_retry`, `abstain`) como campo de salida, no como efecto oculto.

### Medicion de overhead

31. Benchmark A/B en el MISMO proceso y modelo: hooks off vs hooks on, alternando corridas para evitar drift termico.
32. Warmup explicito: al menos 3 requests completos antes de medir (vLLM tiene `--num-warmups 0` por default).
33. Longitud fija: `max_tokens` fijo e `ignore_eos` donde exista. Prompts fijos con semilla.
34. Reportar TTFT, TPOT/ITL y tok/s con p50 y p95 (y p99 en serving).
35. Reportar overhead relativo = (p50_on - p50_off) / p50_off, con intervalo por bootstrap.
36. En CPU fijar `torch.set_num_threads` y reportarlo. Repetir N >= 5 corridas.
37. Medir por separado prefill y decode. El hook pesa distinto en cada uno.
38. Gate de CI: el test de overhead corre en el modelo tiny y falla si supera un umbral relativo generoso. El numero real < 5% se mide en GPU y se documenta aparte.

## Edge cases y riesgos

- gpt-oss renormaliza softmax sobre top-k. La entropia de esos pesos tiene maximo `log(k)`, no `log(n_experts)`. Mezclarlas rompe la calibracion.
- Modelos con gating sigmoid (por ejemplo DeepSeek-V3 con bias de seleccion) no tienen "distribucion" natural. La entropia requiere normalizar. Hay que definir la formula por familia.
- Expertos compartidos (Gemma-4 con 128+1, DeepSeek, Qwen2-MoE) no pasan por el router. La entropia los ignora.
- `router_logits` de transformers puede venir post-softmax segun el docstring. Doble softmax aplana la entropia sin error visible.
- Batching: con padding a izquierda, los hooks ven tokens de pad. Hay que enmascarar con `attention_mask`.
- Prefix caching en vLLM: tokens del prompt cacheados no recomputan el router. Las features del prompt pueden faltar.
- Speculative decoding: tokens rechazados igual pasan por el router. Hay que descartar sus features.
- Preempcion en vLLM: el replay oficial pierde datos de requests preemptados sin aviso. Un capturer propio hereda el riesgo.
- CUDA graphs y `torch.compile`: un hook Python con efectos laterales rompe la captura. El calculo debe ser in-place en buffers fijos.
- Async scheduling en vLLM: el replay oficial lo rechaza. El plugin Vigia probablemente tambien.
- Versionado: la API interna de FusedMoE cambio de `custom_routing_function` a `FusedMoERouter`. El plugin queda atado a versiones de vLLM.
- Tokens multibyte en espanol (tildes, n): un token puede ser medio caracter. Usar `bytes` para reconstruir offsets.
- Streaming con detokenizacion incremental: el texto del `delta` puede no coincidir 1:1 con tokens.
- Windows: vLLM no corre nativo. El dev en la laptop usa HF transformers o llama.cpp. vLLM se prueba en Linux con GPU.
- 8 GB de RAM: gpt-oss-20b no entra. Los tests usan tiny-random-Mixtral y Granite 1B-A400M.
- Cuantizacion: GGUF de 4 bits cuantiza el router en llama.cpp salvo que se excluya. La entropia puede cambiar frente a bf16 [no verificado].
- Multi-tenant: un buffer de features compartido puede filtrar datos entre requests si se indexa mal. Hace falta test de aislamiento.

## Objeciones

- Falta el costo real de calcular entropia dentro de `custom_routing_function` en vLLM. No hay medicion publica.
- Falta saber si el Nivel R solo alcanza AUROC util. InnerExpert reporta 0.76 por token con todas las senales, no por subconjunto en lo que lei.
- El 3x de InnerExpert puede venir de las senales de atencion y hidden states, no del router. No lo pude separar.
- No verifique si `return_routed_experts` de SGLang funciona en streaming ni su flag de server.
- No verifique el costo de `cb_eval` en llama.cpp con GPU (sincronizacion por nodo observado).
- No verifique que `llama-cpp-python` exponga `cb_eval` de forma usable.
- No lei la doc oficial de OpenAI (403). Use los tipos del SDK `openai-python` como fuente.
- No verifique la forma exacta del router de Granite MoE, OLMoE ni Mixtral en transformers v5.
- No verifique si existe un RFC abierto en vLLM para devolver logits del router (ademas de IDs).

## Fuentes

- vLLM routed experts replay: https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
- vLLM RoutedExpertsCapturer: https://docs.vllm.ai/en/latest/api/vllm/model_executor/layers/fused_moe/routed_experts_capturer/
- vLLM ejemplo e2e: https://docs.vllm.ai/en/latest/examples/rl/routed_experts_e2e/
- vLLM FusedMoERouter: https://docs.vllm.ai/en/v0.14.0/api/vllm/model_executor/layers/fused_moe/fused_moe_router/
- vLLM plugins: https://docs.vllm.ai/en/latest/design/plugin_system.html
- vLLM bench serve: https://docs.vllm.ai/en/latest/cli/bench/serve/
- vLLM instalacion: https://docs.vllm.ai/en/latest/getting_started/installation/
- Foro vLLM routing replay: https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
- Foro vLLM probes: https://discuss.vllm.ai/t/implementing-hidden-state-probes/2291
- Issue vLLM #19342: https://github.com/vllm-project/vllm/issues/19342
- Blog vLLM hidden states: https://vllm.ai/blog/2026-03-30-extract-hidden-states
- vLLM Hook v0: https://arxiv.org/html/2603.06588 y https://github.com/IBM/vllm-hook
- SGLang expert_distribution.py: https://raw.githubusercontent.com/sgl-project/sglang/main/python/sglang/srt/eplb/expert_distribution.py
- SGLang io_struct.py: https://raw.githubusercontent.com/sgl-project/sglang/main/python/sglang/srt/managers/io_struct.py
- SGLang native API: https://docs.sglang.io/basic_usage/native_api.html
- llama.cpp ggml-backend.h: https://raw.githubusercontent.com/ggml-org/llama.cpp/master/ggml/include/ggml-backend.h
- llama.cpp llama-graph.cpp: https://raw.githubusercontent.com/ggml-org/llama.cpp/master/src/llama-graph.cpp
- llama.cpp llama-context.cpp: https://raw.githubusercontent.com/ggml-org/llama.cpp/master/src/llama-context.cpp
- llama.cpp eval-callback: https://raw.githubusercontent.com/ggml-org/llama.cpp/master/examples/eval-callback/eval-callback.cpp
- TGI (modo mantenimiento): https://huggingface.co/docs/text-generation-inference/en/index.md
- Ollama v0.12.11: https://github.com/ollama/ollama/releases/tag/v0.12.11
- Transformers gpt-oss: https://huggingface.co/docs/transformers/main/en/model_doc/gpt_oss
- modeling_gpt_oss.py: https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
- modeling_qwen3_moe.py: https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/qwen3_moe/modeling_qwen3_moe.py
- gpt-oss-20b config: https://huggingface.co/openai/gpt-oss-20b/raw/main/config.json
- Granite 3.1 1B-A400M: https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct
- tiny-random-Mixtral: https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM/raw/main/config.json
- Foro HF router logits en generate: https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
- Transformers generation strategies: https://huggingface.co/docs/transformers/main/en/generation_strategies
- openai-python logprob types: https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/chat/chat_completion_token_logprob.py
- openai-python chunk types: https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/chat/chat_completion_chunk.py
- openai-python _models.py: https://raw.githubusercontent.com/openai/openai-python/main/src/openai/_models.py
- openai-python README: https://raw.githubusercontent.com/openai/openai-python/main/README.md
- PyTorch benchmark recipe: https://docs.pytorch.org/tutorials/recipes/recipes/benchmark.html
- InnerExpert: https://arxiv.org/html/2608.17687
