---
title: Vigia - integraciones, politicas de accion y edge cases testeables
mision: Que integraciones necesita Vigia, como modela acciones sobre fragmentos dudosos, y que edge cases tecnicos debe cubrir con comportamiento testeable (dado X, entonces Y)?
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://developers.openai.com/cookbook/articles/openai-harmony
  - https://github.com/openai/harmony
  - https://huggingface.co/openai/gpt-oss-20b
  - https://vllm.ai/blog/gpt-oss
  - https://recipes.vllm.ai/openai/gpt-oss-20b
  - https://huggingface.co/docs/transformers/quantization/mxfp4
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/deepseek_v3/modeling_deepseek_v3.py
  - https://huggingface.co/docs/transformers/model_doc/qwen2_moe
  - https://huggingface.co/docs/transformers/model_doc/deepseek_v3
  - https://huggingface.co/docs/transformers/llm_tutorial
  - https://huggingface.co/docs/transformers/main_classes/tokenizer
  - https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
  - https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct
  - https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM
  - https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
  - https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
  - https://docs.vllm.ai/en/latest/design/plugin_system/
  - https://docs.vllm.ai/en/stable/features/batch_invariance/
  - https://docs.vllm.ai/en/stable/api/vllm/v1/engine/logprobs/
  - https://discuss.vllm.ai/t/how-is-vllm-handling-internal-queue-requests/2615
  - https://thinkingmachines.ai/blog/defeating-nondeterminism-in-llm-inference/
  - https://raw.githubusercontent.com/openai/openai-python/main/README.md
  - https://raw.githubusercontent.com/openai/tiktoken/main/tiktoken/core.py
  - https://reference.langchain.com/python/langchain-openai/langchain_openai/chat_models/base/ChatOpenAI
  - https://developers.llamaindex.ai/python/framework-api-reference/llms/openai_like/
  - https://github.com/Arize-ai/openinference/blob/main/spec/semantic_conventions.md
  - https://arize.com/docs/phoenix/tracing/how-to-tracing/setup-tracing/setup-using-phoenix-otel
  - https://langfuse.com/integrations/native/opentelemetry
  - https://langfuse.com/docs/evaluation/evaluation-methods/scores-via-sdk
  - https://raw.githubusercontent.com/open-telemetry/semantic-conventions-genai/main/docs/gen-ai/gen-ai-spans.md
  - https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/
  - https://raw.githubusercontent.com/standard-webhooks/standard-webhooks/main/spec/standard-webhooks.md
  - https://community.owasp.org/attacks/CSV_Injection
  - https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/String/length
  - https://docs.python.org/3/library/unicodedata.html
  - https://docs.pytorch.org/docs/stable/notes/randomness.html
  - https://arxiv.org/abs/2608.17687
  - https://arxiv.org/html/2608.17687
  - https://arxiv.org/abs/2305.06983
  - https://arxiv.org/abs/2407.18418
created: 2026-10-04
---

## Tronco

Vigia debe ser un proxy compatible con OpenAI que adjunta puntajes por token con offsets de caracter en UTF-8 y UTF-16, solo sobre el canal `final` de harmony, y que dispara acciones por politica declarativa via webhooks firmados y trazas OpenInference, porque ningun servidor (vLLM incluido) expone hoy los logits del router por API.

## Hallazgos

### H1. Acceso a senales del router: el riesgo tecnico esta confirmado

- vLLM no devuelve logits del router por API. Un hilo del foro oficial lo dice asi: vLLM "does not natively support returning router selections" por capa.
- vLLM tiene un flag `--enable-return-routed-experts` (doc de la version v0.21.0). Ese flag devuelve SOLO IDs de expertos. No devuelve pesos ni logits.
- Con solo IDs no se calcula la entropia del router. Con IDs si se calcula un patron de uso de expertos. La dispersion entre expertos necesita las salidas de los expertos, y esas salidas no salen por API.
- vLLM acepta plugins generales (`vllm.general_plugins`). El caso de uso principal de esos plugins es registrar modelos fuera del arbol con `ModelRegistry.register_model()`. El plugin debe ser re-entrante, porque vLLM lo carga en cada proceso.
- vLLM tambien acepta plugins de endpoint (`vllm.endpoint_plugins`). Esos plugins no cargan por defecto.
- Hugging Face Transformers expone `output_router_logits`. En gpt-oss existe un `OutputRecorder(GptOssTopKRouter, index=0)` para `router_logits`.
- En generacion con cache, `output_router_logits=True` falla en algunos modelos. PhiMoE y Mixtral dan un error de forma de tensor. La causa es la perdida auxiliar: el router solo ve los tokens nuevos, pero la mascara de atencion cubre la secuencia entera. Las soluciones del foro son `use_cache=False`, una pasada aparte, o un parche.
- La doc de Transformers dice que los router logits "should not be returned during inference". Vigia no puede depender de ese flag. Vigia debe usar forward hooks propios sobre el modulo router.

| Backend | Logits del router | IDs de expertos | Salidas de expertos | Streaming | Uso en Vigia |
|---|---|---|---|---|---|
| HF Transformers + hook propio | Si (hook en router) | Si | Si (hook en experts) | Si (con streamer propio) | Referencia y tests CPU |
| HF `output_router_logits` | Si, con fallas en generate+cache | Indirecto | No | No directo | Evitar en generacion |
| vLLM `--enable-return-routed-experts` | No | Si, int16 | No | No soportado | Senal parcial (uso de expertos) |
| vLLM + plugin de modelo propio | Si (parche del modelo) | Si | Si | A disenar | Ruta de produccion GPU |
| vLLM sin cambios | No | No | No | Si | Sin senales MoE |

Detalle del flag de vLLM (doc v0.21.0):

| Campo | Forma | Tipo | Nivel |
|---|---|---|---|
| `prompt_routed_experts` | `[prompt_len, num_moe_layers, top_k]` | int16 | respuesta |
| `routed_experts` | `[gen_len, num_moe_layers, top_k]` | int16 | choice |

- Ese flag no soporta streaming, motor V2, paralelismo de secuencia ni de pipeline, ni scheduling asincrono.
- Si el request sufre preempcion, el flag pierde el ruteo acumulado sin aviso.
- La doc reporta un costo de throughput de "~2%" en su configuracion de prueba.
- [no verificado] La ruta exacta de esa pagina en la doc `latest` dio 404. La pagina puede haberse movido o el flag puede haber cambiado.

### H2. Diferencias de router entre familias MoE

La definicion de "entropia del router" cambia segun la familia. Vigia debe normalizar.

| Familia | Funcion de puntaje | Top-k | Expertos compartidos | Capas sin router | Fuente |
|---|---|---|---|---|---|
| gpt-oss-20b | softmax DESPUES del top-k | 4 de 32 | No | Ninguna [no verificado para todas] | modeling_gpt_oss.py, blog vLLM |
| gpt-oss-120b | softmax DESPUES del top-k | 4 de 128 | No | Ninguna [no verificado] | blog vLLM |
| DeepSeek-V3 | sigmoid + `e_score_correction_bias` + top-k por grupos | 8 de 256 (defaults) | 1 (`n_shared_experts`) | primeras `first_k_dense_replace=3` | modeling_deepseek_v3.py, doc HF |
| Qwen2-MoE | softmax (`norm_topk_prob` opcional) | 4 de 60 (defaults) | Si (`shared_expert_intermediate_size`) | `mlp_only_layers`, `decoder_sparse_step` | doc HF |
| Granite 3.1 1B-A400M | [no verificado] | 8 de 32 | [no verificado] | [no verificado] | model card |
| OLMoE-1B-7B | [no verificado] | 8 de 64 | No | [no verificado] | paper InnerExpert |

- En gpt-oss el router devuelve `router_logits` crudos con forma `(num_tokens, num_experts)`. Los `router_scores` son softmax solo sobre los top-k. La entropia sobre `router_scores` no es la entropia del router completo.
- En DeepSeek-V3 el puntaje es sigmoid, no softmax. Los puntajes no suman 1. La entropia necesita una normalizacion explicita.
- DeepSeek-V3 en HF no registra `router_logits` en `_can_record_outputs`. `DeepseekV3ForCausalLM` devuelve `CausalLMOutputWithPast`, sin router logits. Solo un hook propio obtiene la senal.
- En gpt-oss, la config MXFP4 lista `model.layers.*.mlp.router` en `modules_to_not_convert`. El router NO se cuantiza. Las salidas de expertos si estan cuantizadas.
- El paper InnerExpert usa OLMoE-1B-7B-0924-Instruct y Gemma-4-26B-A4B-it. El paper no usa gpt-oss. La transferencia de la AUROC 0.76 a gpt-oss no esta medida.
- El paper extrae senales de todas las capas MoE y solo para tokens generados, no para el prompt.
- El paper etiqueta tokens asi: el juez devuelve spans alucinados como strings, y un token es positivo si su span de caracteres se solapa con un span alucinado. Ese metodo exige un mapeo token->caracteres exacto.
- El paper reporta que el clasificador agrega ~0.7 s, con ~3x sobre la generacion vanilla en total. Esa cifra choca con la meta del plan de < 5% de latencia.

### H3. Formato harmony de gpt-oss

| Token | ID | Rol |
|---|---|---|
| `<\|start\|>` | 200006 | inicio de mensaje |
| `<\|end\|>` | 200007 | fin de mensaje |
| `<\|message\|>` | 200008 | paso de header a contenido |
| `<\|channel\|>` | 200005 | indica canal |
| `<\|constrain\|>` | 200003 | tipo de dato de tool call |
| `<\|return\|>` | 200002 | fin de inferencia (respuesta final) |
| `<\|call\|>` | 200012 | fin por tool call |

- Los canales son `analysis` (cadena de razonamiento), `commentary` (tool calls y preambulos) y `final` (respuesta al usuario).
- La guia de OpenAI advierte que el canal `analysis` no cumple los mismos estandares de seguridad que `final`. La guia pide no mostrarlo al usuario final.
- Al guardar historial, `<|return|>` se reemplaza por `<|end|>`.
- En multi-turno se descarta el razonamiento previo despues de un `final`. Se conserva si hubo tool calls.
- La libreria `openai-harmony` (Rust con bindings Python) renderiza y parsea. Tiene un `StreamableParser` con `current_channel` y `last_content_delta`.
- El model card de gpt-oss dice que el modelo solo funciona bien con harmony.
- vLLM expone el razonamiento en `reasoning_content` en Chat Completions.

### H4. Tokenizacion, bytes y offsets

- tiktoken advierte que `decode` es con perdida: los bytes de un token no son UTF-8 valido por si solos.
- tiktoken ofrece `decode_with_offsets`. Si un token empieza dentro de un caracter multi-byte, el offset es el indice del primer caracter que contiene bytes de ese token.
- vLLM corrige tokens de logprobs que decodifican a U+FFFD. vLLM usa hasta 4 tokens previos como contexto. La doc dice que 4 alcanza para cualquier secuencia UTF-8.
- En UTF-8, "n con tilde" y las vocales con tilde ocupan 2 bytes. Un BPE a nivel byte puede partir ese caracter entre dos tokens [hecho general de UTF-8; la particion concreta en o200k_harmony no esta verificada].
- `return_offsets_mapping` solo existe en tokenizers "fast". En tokenizers Python lanza `NotImplementedError`.
- JavaScript mide `String.length` en unidades UTF-16. Un emoji fuera del BMP cuenta 2. Python mide en code points. El panel web resalta con indices JS, asi que un offset Python desalinea el resaltado despues de un emoji.
- Unicode admite "a con tilde" compuesta (NFC) o descompuesta (NFD). Dos strings iguales a la vista pueden no ser iguales en bytes ni en longitud.

### H5. Integraciones de cliente

| Integracion | Mecanismo | Que preserva | Riesgo para Vigia |
|---|---|---|---|
| OpenAI Python SDK | `base_url` o `OPENAI_BASE_URL`; `extra_body`, `extra_headers` | Campos extra via `response.model_extra` | Bajo: campos propios llegan |
| LangChain `ChatOpenAI` | `base_url`, `extra_body`, `logprobs` en `response_metadata` | NO preserva campos no estandar (ej. `reasoning_content`) | Alto: los puntajes de Vigia se pierden si van en un campo propio |
| LlamaIndex `OpenAILike` | `api_base`, `is_chat_model`, `additional_kwargs`, `default_headers` | Logprobs no documentados | Medio: necesita adaptador |
| Phoenix | OTLP HTTP `:6006/v1/traces` o gRPC `:4317`; `phoenix.otel.register()` | Atributos OpenInference | Bajo |
| Langfuse | OTLP HTTP en `/api/public/otel`; Basic auth `pk:sk`; sin gRPC | `langfuse.*`, `gen_ai.*`, `input.value`/`output.value`; resto a metadata | Medio: atributos no mapeados no se consultan |

- El SDK OpenAI tiene timeout default de 10 minutos y 2 reintentos automaticos. El SDK exige context manager o `.close()` para cerrar streams con seguridad.
- LangChain documenta que con `base_url` hacia vLLM conviene un paquete especifico del proveedor. LangChain pide `extra_body` en lugar de `model_kwargs`.
- LangChain agrega logprobs de chunks al concatenar. Con `stream_usage=True` agrega uso al mensaje final.
- OpenAI Chat Completions devuelve `logprobs.content[]` con `token`, `logprob`, `bytes` y `top_logprobs`. [no verificado en fuente primaria: la referencia de la API devolvio 403; el formato se infiere de la doc de LangChain que menciona representaciones en bytes].
- Langfuse exige que atributos de traza (`userId`, `sessionId`, metadata) esten en cada span, no solo en el raiz.
- Langfuse acepta scores `NUMERIC`, `CATEGORICAL`, `BOOLEAN` y `TEXT` por traza u observacion.
- OpenInference define `openinference.span.kind=LLM`, `llm.input_messages`, `llm.output_messages`, `llm.token_count.*`, `llm.model_name`, `metadata`, `tag.tags`, `session.id`, `user.id`, y `evaluations`. OpenInference no define atributos de logprobs.
- OTel GenAI esta en estado Development. Requiere `gen_ai.operation.name` y `gen_ai.provider.name`. El contenido (`gen_ai.input.messages`, `gen_ai.output.messages`) es opt-in por PII.
- OTel limita por defecto 128 atributos por span y 128 eventos por span. El largo de valor no tiene limite. El batch processor exporta lotes de 512 con cola de 2048 y demora de 5000 ms.

### H6. Webhooks y export

- Standard Webhooks define `webhook-id`, `webhook-timestamp` (unix en segundos) y `webhook-signature`.
- La firma es HMAC-SHA256 sobre `msg_id.timestamp.payload`, en base64, con prefijo `v1,`. El secreto lleva prefijo `whsec_`.
- La spec recomienda `webhook-id` como clave de idempotencia. La spec recomienda payloads menores a 20 kb y timeout de 15 a 30 s. Solo un 2xx cuenta como exito.
- La spec propone reintentos con backoff: inmediato, 5 s, 5 min, 30 min, 2 h, 5 h, 10 h, 14 h, 20 h, 24 h.
- La spec pide verificar el timestamp contra una tolerancia, pero no fija el valor.
- OWASP lista `=`, `+`, `-`, `@`, tab, CR y LF como iniciadores de inyeccion de formulas en CSV. OWASP recomienda comillas dobles, escapar comillas y un prefijo (comilla simple o tab).

### H7. Politicas de accion: antecedentes

- FLARE (arXiv 2305.06983) re-genera la oracion siguiente si contiene tokens de baja confianza. FLARE arma la consulta enmascarando esos tokens. Es el antecedente directo de "re-consulta con RAG por span".
- La encuesta "Know Your Limits" (arXiv 2407.18418, TACL) define abstencion como la negativa del LLM a responder. La encuesta mira la abstencion desde la consulta, el modelo y los valores humanos.

### H8. Determinismo y concurrencia

- Thinking Machines midio 1000 completions de Qwen3-235B a temperatura 0. Obtuvo 80 completions unicas. Todas coincidieron en los primeros 102 tokens. La causa es la falta de invariancia de batch, no solo la no-asociatividad de punto flotante.
- vLLM ofrece `VLLM_BATCH_INVARIANT=1`. Requiere GPU NVIDIA con capacidad 8.0 o mas (o XPU Intel con Triton). La lista probada incluye Qwen3-MoE y GPT-OSS. El modo cuesta rendimiento.
- PyTorch no garantiza reproducibilidad entre versiones, plataformas ni entre CPU y GPU. PyTorch ofrece `torch.manual_seed` y `torch.use_deterministic_algorithms`.
- En MoE, una diferencia numerica minima en el router puede cambiar el top-k elegido. Eso cambia la salida y las senales [inferencia; no medido].

### H9. Cancelacion, timeouts y memoria

- vLLM aborta el request cuando detecta la desconexion HTTP. Un request en cola se cancela antes de ejecutar. Un request en curso puede terminar el paso actual antes de abortar.
- vLLM no ofrece un endpoint para abortar por ID; la cancelacion ocurre por desconexion.
- gpt-oss-20b ocupa unos 14 GB en MXFP4. Los kernels MXFP4 de Transformers requieren GPU NVIDIA con capacidad 7.5 o mas y Triton 3.4 o mas. La laptop de build (8 GB, sin GPU) no puede correr gpt-oss-20b.
- Modelos MoE chicos para tests en CPU: `hf-internal-testing/tiny-random-MixtralForCausalLM` (pesos aleatorios) y `ibm-granite/granite-3.1-1b-a400m-instruct` (1.3B total, 400M activos, 32 expertos, top-8, Apache 2.0, incluye espanol).
- Transformers genera 20 tokens por defecto si no se fija `max_new_tokens`.
- Para batch, los modelos decoder-only necesitan padding a la izquierda. Con padding a la derecha la salida se degrada (ejemplo de la doc: "1, 2, 33333333333").

## Implicancias para Vigia

### I1. Arquitectura de extraccion

1. Implementar un `SignalExtractor` con forward hooks sobre el modulo router de cada capa MoE. No usar `output_router_logits`.
2. Definir un adaptador por familia (`RouterAdapter`) con tres metodos: `find_routers(model)`, `router_probs(logits)`, `expert_outputs(...)`.
3. Normalizar la entropia: `H = -sum(p log p) / log(num_experts)`, con `p = softmax(router_logits)` sobre TODOS los expertos. Reportar ademas la entropia sobre top-k como senal separada.
4. Para DeepSeek-V3, calcular `p = sigmoid(logits) / sum(sigmoid(logits))` y documentarlo como variante.
5. Excluir capas densas (`first_k_dense_replace`, `mlp_only_layers`, `decoder_sparse_step`). Registrar en el reporte cuantas capas MoE se usaron.
6. Extraer senales solo de tokens generados, como el paper. El prompt no entra al puntaje por defecto.
7. Ofrecer tres modos de backend con capacidades declaradas: `hf-hooks` (senales completas), `vllm-plugin` (senales completas, GPU), `vllm-routed-experts` (solo uso de expertos). El detector debe saber que features faltan.

### I2. Contrato de salida por token (compartido entre modulos)

Cada token emitido lleva este registro:

| Campo | Tipo | Nota |
|---|---|---|
| `index` | int | posicion en la salida generada |
| `token_id` | int | ID del tokenizer |
| `bytes` | list[int] | bytes crudos del token |
| `text` | str | texto decodificado con contexto (sin U+FFFD si se puede corregir) |
| `char_start_cp`, `char_end_cp` | int | offsets en code points sobre el texto final normalizado |
| `char_start_u16`, `char_end_u16` | int | offsets en unidades UTF-16 para el panel |
| `byte_start`, `byte_end` | int | offsets en bytes UTF-8 |
| `channel` | str | `final`, `analysis`, `commentary` o `none` |
| `is_special` | bool | token especial del template |
| `score` | float o null | probabilidad calibrada de alucinacion; null para especiales |
| `features` | dict | entropia, dispersion, uso de expertos por capa agregada |

### I3. API e integraciones

1. Exponer un proxy OpenAI-compatible (`/v1/chat/completions`, `/v1/completions`) con streaming SSE.
2. Poner los puntajes en DOS lugares: un campo propio `vigia` en la respuesta y un encabezado `X-Vigia-Trace-Id`. El campo propio sirve al SDK OpenAI via `model_extra`.
3. Publicar un cliente Python `vigia.Client` que envuelve `openai.OpenAI` y devuelve objetos tipados.
4. Publicar un callback de LangChain y un wrapper de LlamaIndex. Ambos deben consultar los puntajes por `trace_id` al endpoint `/v1/vigia/traces/{id}`, porque LangChain descarta campos no estandar.
5. Exportar trazas OTLP con atributos OpenInference y `gen_ai.*`. Agregar `vigia.*` para resumenes: `vigia.score.max`, `vigia.score.mean`, `vigia.flagged_spans`, `vigia.policy.action`.
6. No poner arrays por token como atributos de span: el limite default es 128 atributos. Poner el detalle por token en un evento comprimido o en el store de Vigia, y un link en el span.
7. Para Langfuse, repetir `langfuse.session.id` y `langfuse.user.id` en cada span. Enviar el puntaje de respuesta como score `NUMERIC` llamado `vigia_confidence`.
8. Contenido de mensajes en trazas: apagado por defecto (opt-in), como pide OTel GenAI, por PII en finanzas, salud y legal.

### I4. Politicas de accion

1. Definir la politica como documento declarativo versionado (YAML o JSON) por tenant y por ruta.
2. Niveles de agregacion: token -> span -> respuesta. Un span es una corrida maxima de tokens contiguos con `score >= umbral_token`, con union de huecos de hasta `gap_max` tokens y largo minimo `min_tokens`.
3. Expandir el span a limites de palabra para el resaltado. Nunca cortar un caracter.
4. Reglas: `when` (condicion sobre span o respuesta) -> `action` (`pass`, `annotate`, `human_review`, `rag_retry`, `abstain`). Prioridad: `abstain` > `rag_retry` > `human_review` > `annotate` > `pass`.
5. Condiciones de respuesta: `max_score`, `mean_score`, `flagged_ratio`, `n_spans`. Condiciones de span: `span_max`, `span_len`.
6. Semantica de `abstain`: la respuesta al cliente es un mensaje de abstencion configurable. El texto original queda en el store para auditoria. `finish_reason` se mantiene y se agrega `vigia.action="abstain"`.
7. Semantica de `rag_retry`: Vigia arma la consulta enmascarando los tokens dudosos (como FLARE). Vigia llama al webhook de recuperacion. Vigia re-genera con el contexto recuperado. Maximo `max_retries` (default 1). Si el reintento sigue sobre el umbral, se aplica la accion de respaldo (`abstain` o `human_review`).
8. Semantica de `human_review`: la respuesta se entrega marcada como pendiente o se retiene, segun `mode: async|blocking`. El webhook crea un item de revision con link al panel.
9. En streaming, las politicas de respuesta solo se deciden al final. Ofrecer `stream_policy: buffer|annotate_only`. `buffer` retiene la salida hasta decidir.
10. Toda accion queda en un log de auditoria con `policy_version`, `trace_id`, puntajes y decision.

### I5. Webhooks

1. Seguir Standard Webhooks: encabezados `webhook-id`, `webhook-timestamp`, `webhook-signature`; HMAC-SHA256 `v1,`; secretos `whsec_`.
2. Tipos de evento: `vigia.span.flagged`, `vigia.response.abstained`, `vigia.review.requested`, `vigia.rag.retry_requested`, `vigia.rag.retry_completed`.
3. Payload menor a 20 kb. Mandar referencia y resumen, no el texto completo, salvo opt-in.
4. Timeout de 15 s. Reintentos con el calendario de la spec. Cola persistente.
5. Tolerancia de timestamp: 5 minutos (decision de diseno; la spec no fija valor).
6. Proteger contra SSRF: bloquear destinos privados y loopback salvo allowlist explicita [decision de diseno].

### I6. Export CSV/JSON

1. JSON: export por traza con el contrato de I2, mas `schema_version`.
2. JSONL para lotes grandes. Streaming en el servidor para no cargar todo en memoria.
3. CSV: UTF-8 con BOM opcional para Excel. Escapar comillas. Neutralizar celdas que empiezan con `=`, `+`, `-`, `@`, tab, CR o LF segun OWASP.
4. Incluir offsets en code points y en UTF-16 para que cualquier consumidor reconstruya el resaltado.

### I7. Harmony

1. Parsear la salida de gpt-oss con `openai-harmony` (`StreamableParser`). No parsear con regex.
2. Puntuar solo el canal `final` por defecto. Calcular senales de `analysis` como diagnostico interno, nunca mostradas al usuario final.
3. Marcar tokens de `commentary` (tool calls) como no puntuables por defecto.
4. Marcar todos los tokens especiales (IDs 200002-200012 de la tabla) con `is_special=true` y `score=null`.

### I8. Calibracion y reporte

1. El etiquetado del juez usa el mismo mapeo de offsets que el panel. Un token es positivo si su span de caracteres se solapa con un span alucinado (metodo del paper).
2. El reporte AUROC debe fallar con mensaje claro si el set tiene una sola clase.
3. El reporte de latencia debe medir sobrecarga contra generacion sin Vigia, con la misma semilla y el mismo batch.

### I9. Tests en CPU

1. Usar `tiny-random-MixtralForCausalLM` para tests de forma, hooks y contratos (rapidos, pesos aleatorios).
2. Usar `granite-3.1-1b-a400m-instruct` para tests de integracion con espanol real en CPU.
3. Simular gpt-oss con un modelo chico de la misma clase (config reducida de `GptOssForCausalLM` con pesos aleatorios) para probar el adaptador [no verificado que la config reducida cargue en CPU sin kernels MXFP4; sin cuantizacion deberia cargar en bf16/fp32].
4. Simular harmony con el tokenizer o200k_harmony y secuencias sinteticas de tokens.

## Edge cases y riesgos

Formato: **EC-ID. Dado X, entonces Y.** Cada item es un test.

### Modelo y arquitectura

- **EC-01.** Dado un modelo denso (ej. Llama), entonces `Vigia.attach(model)` lanza `UnsupportedModelError` con el texto "no se encontraron capas MoE" y la lista de clases buscadas. No devuelve puntajes vacios.
- **EC-02.** Dado un modelo MoE con capas densas mezcladas (DeepSeek-V3 con `first_k_dense_replace=3`), entonces el extractor registra solo las capas con router y el reporte indica `moe_layers=N`.
- **EC-03.** Dado un modelo con expertos compartidos (Qwen2-MoE, DeepSeek), entonces la dispersion entre expertos excluye el experto compartido y lo documenta en `features.meta`.
- **EC-04.** Dado un router con sigmoid (DeepSeek-V3), entonces la entropia usa la normalizacion sigmoid documentada y el test compara contra un calculo manual con tolerancia 1e-5.
- **EC-05.** Dado gpt-oss, entonces la entropia se calcula sobre `softmax(router_logits)` completo y no sobre `router_scores` (top-k).
- **EC-06.** Dado un modelo con arquitectura desconocida pero con un modulo `*Router*` o `gate`, entonces Vigia falla cerrado con `UnsupportedModelError` salvo que el usuario registre un adaptador.
- **EC-07.** Dado un detector calibrado para el modelo A, cuando se carga con el modelo B (distinto `num_experts` o `num_layers`), entonces Vigia rechaza el detector con `CalibrationMismatchError`.
- **EC-08.** Dado un checkpoint cuantizado (MXFP4, GPTQ, AWQ, int8), entonces Vigia registra `quantization` en los metadatos y exige un detector calibrado con la misma cuantizacion; si no coincide, emite advertencia y marca `calibration_valid=false`.
- **EC-09.** Dado gpt-oss MXFP4, entonces el test verifica que el router no esta cuantizado (`modules_to_not_convert` incluye el router) y que los hooks leen tensores de punto flotante.
- **EC-10.** Dado `torch.compile` o kernels fusionados que saltean el modulo router, entonces el extractor detecta cero llamadas al hook en el primer paso y lanza `HookNotFiredError`.

### Tokenizacion y offsets

- **EC-11.** Dado el texto "La niñez, el año y la acción" (con ñ y tildes reales), entonces la concatenacion de `text` de todos los tokens es igual al texto final, byte a byte.
- **EC-12.** Dado un token BPE que contiene solo el primer byte de "n con tilde", entonces su `text` no contiene U+FFFD en la salida final, y su `char_start/char_end` apunta al caracter completo compartido con el token siguiente.
- **EC-13.** Dado un caracter partido entre dos tokens con puntajes 0.2 y 0.9, entonces el caracter se resalta con el puntaje maximo (0.9). La regla es max sobre tokens que tocan el caracter.
- **EC-14.** Dado un emoji fuera del BMP antes de un span dudoso, entonces `char_start_u16` difiere de `char_start_cp` en 1 y el panel resalta exactamente las letras correctas.
- **EC-15.** Dado un texto con "a con tilde" en NFD (letra + acento combinante), entonces Vigia no normaliza el texto generado; los offsets se calculan sobre el texto tal cual se entrega y el test verifica el resaltado en NFC y NFD.
- **EC-16.** Dado un tokenizer "slow" sin `return_offsets_mapping`, entonces Vigia calcula offsets por decodificacion incremental de bytes y no lanza `NotImplementedError`.
- **EC-17.** Dado un token de espacio inicial (ej. " Buenos"), entonces el span resaltado empieza en la letra y no en el espacio, salvo que el usuario pida offsets crudos.
- **EC-18.** Dado un span dudoso que termina a mitad de palabra, entonces el resaltado se expande al limite de palabra y conserva los offsets crudos en `raw_span`.
- **EC-19.** Dado texto con saltos de linea `\r\n`, entonces los offsets cuentan ambos caracteres y el export CSV no rompe filas.

### Chat templates y harmony

- **EC-20.** Dada una salida harmony con `analysis` y luego `final`, entonces solo los tokens del canal `final` reciben `score` no nulo y el panel solo muestra `final`.
- **EC-21.** Dada una salida que termina en `<|call|>` (tool call), entonces la respuesta tiene cero tokens `final`, `vigia.status="no_final_channel"` y ninguna politica de respuesta se dispara.
- **EC-22.** Dada una salida que termina en `<|return|>`, entonces el historial guardado reemplaza `<|return|>` por `<|end|>`.
- **EC-23.** Dados tokens especiales (`<|start|>`, `<|channel|>`, `<|message|>`, `<|end|>`), entonces tienen `is_special=true`, `score=null` y no cuentan en `mean_score` ni en `flagged_ratio`.
- **EC-24.** Dada una salida harmony truncada por `max_tokens` dentro del canal `analysis`, entonces `vigia.status="truncated_before_final"` y la politica aplica la accion configurada para respuestas vacias (default `abstain`).
- **EC-25.** Dado un modelo no harmony (Mixtral, Qwen), entonces el canal es `none` para todos los tokens de contenido y los tokens del chat template se marcan especiales.
- **EC-26.** Dado un multi-turno con razonamiento previo, entonces Vigia no puntua tokens del prompt aunque contengan texto `analysis` de turnos anteriores.

### Entradas vacias y limites

- **EC-27.** Dado un prompt vacio (`messages=[]` o contenido ""), entonces el proxy devuelve 400 con codigo `empty_prompt`, sin llamar al modelo.
- **EC-28.** Dada una generacion de 0 tokens (EOS inmediato o `max_tokens=0`), entonces la respuesta tiene `tokens=[]`, `max_score=null`, `vigia.status="empty_generation"`, y la politica aplica la regla de respuesta vacia sin dividir por cero.
- **EC-29.** Dada una generacion de 1 token, entonces el span minimo y los agregados funcionan y `mean_score == max_score`.
- **EC-30.** Dado un prompt mas largo que la ventana del modelo, entonces el proxy devuelve 400 `context_length_exceeded` con el limite y el largo recibido.
- **EC-31.** Dado un contexto largo (ej. 32k tokens) en el limite, entonces la memoria extra de Vigia es proporcional a los tokens generados, no al prompt; el test mide memoria con y sin Vigia en el modelo tiny.
- **EC-32.** Dado `max_new_tokens` ausente en Transformers, entonces Vigia fija un default explicito y no hereda el default de 20 tokens sin avisar.

### Streaming y cancelacion

- **EC-33.** Dado streaming SSE, entonces cada chunk lleva los puntajes de sus tokens y el orden de `index` es estrictamente creciente sin huecos.
- **EC-34.** Dado un token multi-byte partido entre dos chunks SSE, entonces el primer chunk no emite U+FFFD; el texto se retiene hasta completar el caracter y los offsets siguen correctos.
- **EC-35.** Dado que el cliente cierra la conexion a mitad del stream, entonces Vigia cancela la generacion, libera los buffers de senales en menos de 1 s (en el modelo tiny), y guarda la traza con `status="cancelled"` y los tokens emitidos.
- **EC-36.** Dada una cancelacion, entonces ningun webhook de politica de respuesta se dispara; los webhooks de span ya emitidos quedan.
- **EC-37.** Dado `stream_policy=buffer` con accion `abstain`, entonces el cliente no recibe ningun token del texto original.
- **EC-38.** Dado `stream_policy=annotate_only`, entonces el cliente recibe el texto en vivo y un evento final `vigia.summary` con la decision.
- **EC-39.** Dado el backend `vllm-routed-experts`, entonces el streaming se rechaza o se degrada a puntajes al final, porque el flag de vLLM no soporta streaming.

### Batching y padding

- **EC-40.** Dado un batch de prompts de largo distinto con padding a la izquierda, entonces los tokens de padding no generan senales y los puntajes de cada fila son iguales (tolerancia 1e-4) a los de esa fila sola.
- **EC-41.** Dado padding a la derecha en un decoder-only, entonces Vigia fuerza padding izquierdo o lanza `PaddingSideError`.
- **EC-42.** Dado un batch donde una fila termina antes, entonces las senales de los pasos posteriores de esa fila se descartan.
- **EC-43.** Dado que el mismo request corre solo y dentro de un batch en vLLM sin batch invariance, entonces el reporte documenta la diferencia; el test de determinismo solo se exige en CPU con batch fijo.

### Determinismo

- **EC-44.** Dada semilla fija, greedy, CPU y batch 1, entonces dos corridas producen los mismos tokens y los mismos puntajes exactos.
- **EC-45.** Dado `temperature>0` sin semilla, entonces Vigia registra `seed=null` en la traza y no promete reproducibilidad.
- **EC-46.** Dado un cambio de version de torch o de hardware, entonces el reporte de calibracion registra versiones y advierte que los puntajes pueden variar.

### Concurrencia, timeouts y recursos

- **EC-47.** Dados 20 requests concurrentes, entonces las senales de un request nunca aparecen en otro. El test usa prompts con marcas unicas y verifica el aislamiento por `trace_id`.
- **EC-48.** Dados dos tenants, entonces un tenant no puede leer trazas, politicas ni exports del otro (403), aunque adivine el `trace_id`.
- **EC-49.** Dado un request que supera `request_timeout`, entonces el proxy devuelve 504, cancela la generacion y guarda la traza parcial con `status="timeout"`.
- **EC-50.** Dado un OOM durante la generacion, entonces el proxy devuelve 503 `resource_exhausted`, libera hooks y buffers, y el siguiente request funciona.
- **EC-51.** Dado que la cola supera `max_queue`, entonces el proxy devuelve 429 con `Retry-After`.
- **EC-52.** Dado que el detector falla (excepcion), entonces la politica `on_detector_error` decide: `fail_open` entrega el texto con `score=null` y `status="detector_error"`; `fail_closed` abstiene. El default para sectores regulados es `fail_closed`.

### Politicas

- **EC-53.** Dado un span con `score=0.81` y umbral 0.8, entonces el span se marca. Dado `score=0.8` exacto, entonces se marca (`>=`). El test fija la semantica del limite.
- **EC-54.** Dados dos spans separados por 1 token y `gap_max=2`, entonces se unen en un span.
- **EC-55.** Dadas reglas que disparan `human_review` y `abstain` a la vez, entonces gana `abstain` por prioridad y el log registra ambas coincidencias.
- **EC-56.** Dado `rag_retry` con `max_retries=1` y un reintento que sigue sobre el umbral, entonces se aplica la accion de respaldo y no hay un segundo reintento.
- **EC-57.** Dado que el webhook de recuperacion RAG falla o vence, entonces se aplica la accion de respaldo y la traza registra `rag_status="error"`.
- **EC-58.** Dada una politica invalida (umbral fuera de [0,1], accion desconocida), entonces la API la rechaza con 422 y la politica activa no cambia.
- **EC-59.** Dado un cambio de politica en caliente, entonces los requests en curso terminan con la version vieja y los nuevos usan la nueva; cada traza registra `policy_version`.

### Webhooks y export

- **EC-60.** Dado un webhook con firma valida, entonces el receptor de ejemplo lo acepta. Dado un byte cambiado en el payload, entonces lo rechaza.
- **EC-61.** Dado un `webhook-timestamp` con 6 minutos de antiguedad, entonces el receptor lo rechaza por replay.
- **EC-62.** Dado un destino que responde 500, entonces Vigia reintenta con el calendario configurado; dado un 2xx, entonces marca entregado; el mismo `webhook-id` se repite en cada reintento.
- **EC-63.** Dado un destino en `127.0.0.1` o `169.254.169.254` sin allowlist, entonces Vigia rechaza la URL al registrarla.
- **EC-64.** Dado un texto generado que empieza con `=HYPERLINK(...)`, entonces el CSV exportado neutraliza la celda y el JSON lo guarda sin cambios.
- **EC-65.** Dado un export de 100k tokens, entonces el servidor transmite el archivo en streaming y la memoria del proceso no crece en proporcion lineal al export completo.

### Calibracion y metricas

- **EC-66.** Dado un set de calibracion con una sola clase, entonces el reporte AUROC falla con `SingleClassError` y un mensaje en espanol, sin devolver NaN silencioso.
- **EC-67.** Dado un span del juez que no aparece literal en la respuesta (el juez parafrasea), entonces ese span se descarta, se cuenta en `judge_unmatched_spans` y el reporte lo muestra.
- **EC-68.** Dado un span del juez que aparece dos veces en la respuesta, entonces la regla de desempate esta documentada (marcar todas las apariciones) y testeada.
- **EC-69.** Dado un reporte de latencia, entonces compara contra una corrida base sin hooks con mismo modelo, semilla y batch, y reporta p50 y p95.

## Objeciones

- Falta medir si las senales del router de gpt-oss predicen alucinacion. El paper usa OLMoE y Gemma-4, no gpt-oss.
- Falta medir el costo real de los hooks en gpt-oss en GPU. La meta de < 5% choca con el ~3x total que reporta el paper con su clasificador. La laptop de build no puede medirlo.
- Falta confirmar el estado actual de `--enable-return-routed-experts` en la version `latest` de vLLM. La pagina de `latest` dio 404.
- Falta confirmar si los logprobs de vLLM con gpt-oss incluyen tokens del canal `analysis` en Chat Completions.
- Falta confirmar la particion concreta de "n con tilde" y vocales con tilde en o200k_harmony. Se puede medir offline con tiktoken en CPU.
- Falta confirmar la funcion de puntaje del router de Granite MoE y OLMoE en el codigo de Transformers.
- Falta confirmar el formato exacto de `logprobs.content[].bytes` de OpenAI en fuente primaria (la referencia de la API devolvio 403).
- Falta confirmar el comportamiento actual de `sklearn.metrics.roc_auc_score` con una sola clase (error o advertencia con NaN). Vigia debe validar antes de llamar, sin depender de sklearn.
- La regla "max sobre tokens que tocan el caracter" (EC-13) es una decision de diseno. No hay fuente que la valide frente a alternativas.
- La tolerancia de 5 minutos para webhooks es una decision de diseno. La spec no fija valor.

## Fuentes

- Harmony: https://developers.openai.com/cookbook/articles/openai-harmony , https://github.com/openai/harmony
- gpt-oss: https://huggingface.co/openai/gpt-oss-20b , https://vllm.ai/blog/gpt-oss , https://recipes.vllm.ai/openai/gpt-oss-20b
- MXFP4 en Transformers: https://huggingface.co/docs/transformers/quantization/mxfp4
- Codigo HF: modeling_gpt_oss.py y modeling_deepseek_v3.py (raw.githubusercontent.com/huggingface/transformers/main)
- Docs HF: qwen2_moe, deepseek_v3, llm_tutorial, tokenizer
- Foro HF router logits en generate: https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
- Modelos de test: https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct , https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM
- vLLM: routed experts replay (v0.21.0), plugin system, batch invariance, logprobs engine, foro (abort y routing)
- Determinismo: https://thinkingmachines.ai/blog/defeating-nondeterminism-in-llm-inference/ , https://docs.pytorch.org/docs/stable/notes/randomness.html
- SDKs: openai-python README, tiktoken core.py, LangChain ChatOpenAI reference, LlamaIndex OpenAILike reference
- Observabilidad: OpenInference spec, Phoenix OTEL setup, Langfuse OTel y scores, OTel GenAI spans, OTel SDK env vars
- Webhooks: Standard Webhooks spec
- Export y texto: OWASP CSV Injection, MDN String.length, Python unicodedata
- Papers: InnerExpert arXiv 2608.17687, FLARE arXiv 2305.06983, Know Your Limits arXiv 2407.18418
