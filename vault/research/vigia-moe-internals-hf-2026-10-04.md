---
title: Vigia - internals MoE en HF transformers y adaptadores de extraccion de senales
mision: Como extraer router logits y salidas por experto de cada arquitectura MoE de HF transformers sin modificar el modelo, y con que modelos se testea en CPU con 8 GB.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/mixtral/modeling_mixtral.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/qwen2_moe/modeling_qwen2_moe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/qwen3_moe/modeling_qwen3_moe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/olmoe/modeling_olmoe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/granitemoe/modeling_granitemoe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/deepseek_v3/modeling_deepseek_v3.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/phimoe/modeling_phimoe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/llama4/modeling_llama4.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/configuration_gpt_oss.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/mixtral/configuration_mixtral.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/qwen2_moe/configuration_qwen2_moe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/olmoe/configuration_olmoe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/deepseek_v3/configuration_deepseek_v3.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/integrations/moe.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/integrations/mxfp4.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/utils/output_capturing.py
  - https://raw.githubusercontent.com/huggingface/transformers/v4.57.1/src/transformers/models/mixtral/modeling_mixtral.py
  - https://raw.githubusercontent.com/huggingface/transformers/v4.57.1/src/transformers/models/gpt_oss/modeling_gpt_oss.py
  - https://huggingface.co/docs/transformers/experts_interface
  - https://huggingface.co/docs/transformers/quantization/mxfp4
  - https://huggingface.co/blog/faster-transformers
  - https://pypi.org/pypi/transformers/json
  - https://huggingface.co/openai/gpt-oss-20b
  - https://huggingface.co/openai/gpt-oss-20b/raw/main/config.json
  - https://huggingface.co/api/models/openai/gpt-oss-20b?blobs=true
  - https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct
  - https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct/raw/main/config.json
  - https://huggingface.co/api/models/ibm-granite/granite-3.1-1b-a400m-instruct?blobs=true
  - https://huggingface.co/api/models/ibm-granite/granite-3.1-3b-a800m-instruct?blobs=true
  - https://huggingface.co/api/models/allenai/OLMoE-1B-7B-0125-Instruct?blobs=true
  - https://huggingface.co/api/models/Qwen/Qwen1.5-MoE-A2.7B?blobs=true
  - https://huggingface.co/api/models/Isotonic/TinyMixtral-4x248M-MoE?blobs=true
  - https://huggingface.co/api/models/tiny-random/gpt-oss-bf16?blobs=true
  - https://huggingface.co/api/models?search=tiny-random-gpt-oss&limit=50
  - https://huggingface.co/api/models?search=tiny-random-Mixtral&limit=50
  - https://huggingface.co/api/models?search=TinyMixtral&limit=30&sort=downloads
  - https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
  - https://arxiv.org/html/2608.17687
  - https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/main/moeuncert/forwards/_model_forwards.py
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/main/requirements.txt
created: 2026-10-04
---

# Vigia - internals MoE en HF transformers

## Tronco

En transformers v5 todo bloque MoE tiene un router lineal y un modulo de expertos fusionado (tensores 3D), asi que Vigia debe recalcular los router logits con un forward pre-hook sobre el bloque MoE (barato y robusto, incluso con MXFP4) y obtener las salidas por experto con un backend propio registrado en `ExpertsInterface`, porque ya no existen modulos por experto a los que colgar hooks.

## Hallazgos

### H1. Version de transformers y cambio estructural v4 -> v5

- La ultima version en PyPI es **5.18.0** (consultado 2026-10-04). Requiere Python >= 3.10.
- La doc de `experts_interface` corresponde a v5.17.0. El codigo leido es la rama `main`.
- En **v4.57.1**, Mixtral guardaba el bloque en `self.block_sparse_moe`. El gate era un `nn.Linear`. Los expertos eran un `nn.ModuleList` de `MixtralBlockSparseTop2MLP` (w1, w2, w3). El bloque devolvia `(final_hidden_states, router_logits)`.
- En **v5 (main)**, Mixtral guarda el bloque en `self.mlp`. El gate es `MixtralTopKRouter` (un `nn.Parameter` `weight`, no `nn.Linear`). Los expertos son `MixtralExperts` con `gate_up_proj` de shape `[E, 2*I, H]` y `down_proj` de shape `[E, H, I]`. El bloque devuelve solo `hidden_states`.
- Consecuencia: en v5 **no hay un `nn.Module` por experto**. Un forward hook por experto ya no es posible. Vigia necesita dos caminos de adaptador (v4 y v5) o fijar `transformers>=5`.
- InnerExpert (el paper de referencia) fija `transformers>=5.8.0` y `torch>=2.10.0` en su `requirements.txt`.

### H2. Tabla de arquitecturas en transformers v5 (main)

Notacion: E = numero de expertos, k = top-k, N = tokens aplanados (batch*seq), H = hidden size.

| Arquitectura | Atributo del bloque en la capa | Clase bloque MoE | Clase router | Ruta del router | Retorno del router | Orden softmax/top-k | Recorder HF de router_logits |
|---|---|---|---|---|---|---|---|
| GptOss | `layers[i].mlp` | `GptOssMLP` (decorado `@use_kernel_forward_from_hub("MegaBlocksMoeMLP")`) | `GptOssTopKRouter` (weight + **bias**) | `mlp.router` | `(router_logits, router_scores, router_indices)` | **top-k primero**, softmax sobre los k valores | `OutputRecorder(GptOssTopKRouter, index=0)` |
| Mixtral | `layers[i].mlp` | `MixtralSparseMoeBlock` | `MixtralTopKRouter` | `mlp.gate` | `(router_logits, router_scores, router_indices)` | softmax sobre E (float32), top-k, renormaliza | `OutputRecorder(MixtralTopKRouter, index=0)` |
| Qwen2-MoE | `layers[i].mlp` (si es capa sparse) | `Qwen2MoeSparseMoeBlock` (+ `shared_expert` y `shared_expert_gate` sigmoide) | `Qwen2MoeTopKRouter` | `mlp.gate` | `(logits, scores, indices)` | softmax sobre E, top-k, renormaliza si `norm_topk_prob` | `OutputRecorder(Qwen2MoeTopKRouter, index=0)` |
| Qwen3-MoE | `layers[i].mlp` (si es capa sparse) | `Qwen3MoeSparseMoeBlock` | `Qwen3MoeTopKRouter` | `mlp.gate` | `(logits, scores, indices)` | igual que Qwen2-MoE, sin shared expert | `OutputRecorder(Qwen3MoeTopKRouter, index=0)` |
| OLMoE | `layers[i].mlp` | `OlmoeSparseMoeBlock` | `OlmoeTopKRouter` | `mlp.gate` | `(logits, scores, indices)` | softmax sobre E, top-k, renormaliza si `norm_topk_prob` (default False) | `OutputRecorder(OlmoeTopKRouter, index=0)` |
| GraniteMoe | `layers[i].block_sparse_moe` | `GraniteMoeMoE` | `GraniteMoeTopKRouter` | `block_sparse_moe.router` | `(top_k_index, top_k_weights, router_logits)` **orden distinto** | top-k sobre logits, softmax sobre los k | `OutputRecorder(GraniteMoeTopKRouter, index=2)` |
| DeepSeek-V3 | `layers[i].mlp` (capas `>= first_k_dense_replace`) | `DeepseekV3MoE` (+ `shared_experts`) | `DeepseekV3TopkRouter` | `mlp.gate` | `(router_logits, topk_weights, topk_indices)` | **sigmoide** por experto, `+ e_score_correction_bias`, top-k por grupos (`n_group`, `topk_group`), renormaliza, `* routed_scaling_factor` | **no tiene** recorder de router_logits |
| Phi-MoE | `layers[i].mlp` | `PhimoeSparseMoeBlock` | `PhimoeTopKRouter` (subclase de `nn.Linear`) | `mlp.router` | `(router_logits, routing_weights, selected_experts)` | `sparsemixer`: top-2 secuencial con umbral por jitter, softmax sobre logits enmascarados | `OutputRecorder(PhimoeTopKRouter, index=0)` |
| Llama4 (texto) | `layers[i].feed_forward` (si `layer_idx in config.moe_layers`) | `Llama4TextMoe` (+ `shared_expert`) | `Llama4Router` (subclase de `nn.Linear`) | `feed_forward.router` | `(router_scores, router_logits)` | top-k sobre logits, **sigmoide** sobre los k, resto -inf -> 0 | clase pelada `Llama4TextMoe` -> index 1 = router_logits |

Detalles verificados en el codigo:

- Los routers de v5 aplanan la entrada a `(N, H)`. Los router logits salen con shape **`(N, E)`**, no `(B, T, E)`. Vigia debe re-dar forma con el `(B, T)` de la entrada.
- GptOss: el router de main no hace reshape propio. `GptOssMLP.forward` aplana antes de llamarlo. El MLP devuelve `(hidden_states, router_scores)` y la capa descarta el segundo valor.
- GptOss usa una activacion propia: `alpha = 1.702`, `limit = 7.0`, gate y up intercalados (`gate_up[..., ::2]`, `gate_up[..., 1::2]`), salida `(up + 1) * glu`. Los expertos tienen bias.
- GptOss v4.57.1 era distinto: el router devolvia `(router_scores, router_indices)`. `router_scores` era el softmax de los top-k esparcido en un tensor de ceros. El recorder `index=0` entregaba **scores, no logits**. La entropia calculada sobre eso es entropia sobre k, no sobre E.
- Qwen2/Qwen3-MoE: la capa elige MoE solo si `layer_idx not in config.mlp_only_layers` y `(layer_idx + 1) % config.decoder_sparse_step == 0`. Algunas capas pueden ser densas.
- DeepSeek-V3: el router calcula logits en float32. `e_score_correction_bias` es un buffer en fp32 estricto (`_keep_in_fp32_modules_strict`). La seleccion no es softmax.
- Llama4: los expertos se aplican en forma densa. El bloque repite la entrada por cada experto y la multiplica por el score sigmoide (0 para expertos no elegidos).

### H3. Mecanismo oficial de captura (`OutputRecorder` / `capture_outputs`)

- HF instala `register_forward_hook` sobre las clases listadas en `_can_record_outputs`. No reemplaza el forward.
- Se activa con `output_router_logits=True` (kwarg o config).
- Un recorder dado como clase pelada usa `index=0` si la clave contiene `hidden_states`, y `index=1` en otro caso.
- Problema con `generate()`: en el foro de HF se reporta `RuntimeError` de shapes al pedir `output_router_logits=True` con cache KV. La loss auxiliar mezcla logits de un solo paso con el attention mask completo. Las salidas sugeridas son `use_cache=False` (lento) o un segundo forward.
- Conclusion: Vigia **no** debe depender de `output_router_logits`. Debe instalar hooks propios persistentes.

### H4. Backends de expertos (`ExpertsInterface`)

- Backends: `eager`, `batched_mm`, `grouped_mm`, `deepgemm`, `deepgemm_megamoe`, `sonicmoe`.
- En CPU, la doc dice que `grouped_mm` es el mas eficiente para todo tamano. `grouped_mm` requiere PyTorch 2.9+. Si no hay soporte, usa un fallback con loop (`grouped_mm_fallback`).
- `deepgemm`, `deepgemm_megamoe` y `sonicmoe` son solo CUDA.
- Seleccion al cargar: `from_pretrained(..., experts_implementation="eager")`. Cambio en caliente: `model.set_experts_implementation("eager")`. Lectura: `model.get_experts_implementation()`.
- Firma de un backend: `experts_forward(self, hidden_states, top_k_index, top_k_weights) -> Tensor`. `hidden_states` llega aplanado `(N, H)`.
- `ExpertsInterface` hereda de `GeneralInterface`. El patron de registro es `ExpertsInterface.register("nombre", fn)` [no verificado: lei `_global_mapping` y `get_interface`, no el metodo `register`].
- El decorador `use_experts_implementation` guarda en la instancia `has_gate`, `has_bias`, `is_transposed`, `is_concatenated` y agrega `_apply_gate` por defecto si falta. GptOss usa `is_concatenated=False, is_transposed=True, has_bias=True`. Los valores por defecto del decorador sin argumentos [no verificado].

### H5. gpt-oss-20b y MXFP4

| Dato | Valor | Fuente |
|---|---|---|
| Parametros | 21B totales, 3.6B activos | model card |
| Capas / expertos / top-k | 24 capas, 32 expertos, top-4 | config.json |
| hidden_size / intermediate_size | 2880 / 2880 | config.json |
| Atencion | alterna sliding (ventana 128) y full | config.json |
| Pesos en disco | ~13.76 GB (3 shards) | API del hub |
| Dtypes | 1.80B params BF16 + 19.11B en U8 (MXFP4 empaquetado) | API del hub |
| Modulos sin cuantizar | `self_attn`, **`mlp.router`**, `embed_tokens`, `lm_head` | config.json |
| Memoria con MXFP4 | "within 16GB" | model card |
| Requisitos MXFP4 | GPU NVIDIA compute capability >= 7.5, Triton >= 3.4, `kernels`, `accelerate` | doc MXFP4 |
| Fallback sin MXFP4 | bf16, ~4x memoria de MXFP4 | blog HF |
| gpt-oss-120b | defaults de `GptOssConfig`: 36 capas, 128 expertos, top-4; ~80 GB con MXFP4 | config / blog |

Hallazgo critico sobre hooks con MXFP4 (codigo de `integrations/mxfp4.py`):

- `replace_with_mxfp4_linear` reemplaza `GptOssExperts` por `Mxfp4GptOssExperts` y parchea `GptOssMLP.forward` con `MethodType(mlp_forward, module)`.
- El `mlp_forward` parcheado calcula `router_logits = nn.functional.linear(hidden_states, self.router.weight, self.router.bias)`. **No llama a `self.router(...)`.** Un forward hook sobre `GptOssTopKRouter` no se dispara. El recorder de HF tampoco captura nada [inferido del codigo; no probado en ejecucion].
- `Mxfp4GptOssExperts.forward(hidden_states, routing_data, gather_idx, scatter_idx)` usa kernels Triton fusionados con swiglu. Las salidas por experto no son accesibles.
- El kernel MegaBlocks (`use_kernels=True`) no es compatible con MXFP4. Con MegaBlocks la inferencia corre en bf16 y el forward de `GptOssMLP` se reemplaza por el del hub. El comportamiento de hooks con MegaBlocks [no verificado].
- El router queda en BF16 sin cuantizar. Recalcular `F.linear(x, router.weight, router.bias)` desde un pre-hook sobre `GptOssMLP` da los logits exactos.
- En la laptop de build (CPU, 8 GB) gpt-oss-20b **no corre**: sin GPU no hay MXFP4, y en bf16 serian ~42 GB [estimado: 21B x 2 bytes].

### H6. Modelos diminutos con pesos aleatorios (tests offline en CPU)

Todos los configs de v5 son dataclasses. Los nombres de campos estan verificados en los archivos `configuration_*.py`.

| Arquitectura | Config | Campos MoE relevantes |
|---|---|---|
| Mixtral | `MixtralConfig` | `num_local_experts` (8), `num_experts_per_tok` (2), `router_jitter_noise` (0.0) |
| GptOss | `GptOssConfig` | `num_local_experts` (128), `num_experts_per_tok` (4), `head_dim` (64), `layer_types` (se autogenera con largo `num_hidden_layers`), `sliding_window` (128) |
| Qwen2-MoE | `Qwen2MoeConfig` | `num_experts` (60), `num_experts_per_tok` (4), `moe_intermediate_size`, `shared_expert_intermediate_size`, `decoder_sparse_step`, `mlp_only_layers`, `norm_topk_prob` |
| Qwen3-MoE | `Qwen3MoeConfig` | igual a Qwen2-MoE sin shared expert [campos inferidos del modeling] |
| OLMoE | `OlmoeConfig` | `num_experts` (64), `num_experts_per_tok` (8), `norm_topk_prob` (False) |
| GraniteMoe | `GraniteMoeConfig` | `num_local_experts`, `num_experts_per_tok` (del config.json real) |
| DeepSeek-V3 | `DeepseekV3Config` | `n_routed_experts` (alias `num_local_experts`), `n_group`, `topk_group`, `first_k_dense_replace`, `n_shared_experts`, MLA: `q_lora_rank`, `kv_lora_rank`, `qk_rope_head_dim`, `qk_nope_head_dim`, `v_head_dim` |

Ejemplo de patron para un test (no ejecutado; ajustar por arquitectura):

```python
import torch
from transformers import MixtralConfig, MixtralForCausalLM

torch.manual_seed(0)
cfg = MixtralConfig(
    vocab_size=128, hidden_size=64, intermediate_size=128,
    num_hidden_layers=2, num_attention_heads=4, num_key_value_heads=2,
    num_local_experts=4, num_experts_per_tok=2, max_position_embeddings=256,
)
model = MixtralForCausalLM(cfg).eval()   # pesos aleatorios via _init_weights, sin red
ids = torch.randint(0, 128, (2, 7))
```

- Para GptOss hay que cuidar `hidden_size`, `num_attention_heads` y `head_dim` coherentes. `layer_types` se genera solo.
- Para DeepSeek-V3 hay que cuidar que `n_routed_experts % n_group == 0` y que `topk_group <= n_group` [inferido del codigo del router: `view(-1, n_group, E // n_group)` y `topk(2)` por grupo, asi que cada grupo necesita >= 2 expertos].
- Para Qwen2/3-MoE, `decoder_sparse_step=1` y `mlp_only_layers=[]` dan todas las capas MoE.
- Checkpoints aleatorios ya publicados en el hub (requieren red una vez): `tiny-random/gpt-oss-bf16` (13.7 MB de pesos, 6.85M params), `tiny-random/gpt-oss-mxfp4`, `yujiepan/gpt-oss-tiny-random`, `yujiepan/mixtral-tiny-random`, `hf-tiny-v2/tiny-random-MixtralForCausalLM`, `hf-tiny-v2/tiny-random-GptOssForCausalLM`. Ojo: `tiny-random/gpt-oss-bf16` trae un `tokenizer.json` de 27.9 MB.

### H7. Checkpoints MoE reales y si entran en 8 GB de RAM (CPU)

| Checkpoint | Arquitectura | Params totales | Activos | E / k | Pesos en disco | Entra en 8 GB CPU |
|---|---|---|---|---|---|---|
| `ibm-granite/granite-3.1-1b-a400m-instruct` | granitemoe | 1.33B | ~400M | 32 / 8 | 2.67 GB (BF16) | **Si.** Licencia Apache 2.0. Soporta espanol. Contexto 128K. |
| `ibm-granite/granite-3.1-3b-a800m-instruct` | granitemoe | 3.30B | ~800M | [no verificado] | 6.60 GB (BF16) | Justo. En bf16 casi no deja margen. No recomendado en la laptop. |
| `Isotonic/TinyMixtral-4x248M-MoE` | mixtral (merge mergekit) | 701M | [no verificado] | 4 / [no verificado] | 2.80 GB (F32) | **Si** (~1.4 GB si se castea a bf16). Calidad baja; sirve como test de integracion Mixtral. |
| `allenai/OLMoE-1B-7B-0125-Instruct` | olmoe | 6.92B | ~1B | 64 / 8 | 13.84 GB (BF16) | **No.** |
| `Qwen/Qwen1.5-MoE-A2.7B` | qwen2_moe | 14.3B | 2.7B | 60 / 4 | 28.6 GB (BF16) | **No.** |
| `openai/gpt-oss-20b` | gpt_oss | 20.9B | 3.6B | 32 / 4 | 13.76 GB (MXFP4) | **No** (sin GPU no hay MXFP4). |

Config verificado de granite-3.1-1b-a400m: `hidden_size=1024`, `intermediate_size=512`, `num_hidden_layers=24`, `num_local_experts=32`, `num_experts_per_tok=8`, `vocab_size=49155`, `tie_word_embeddings=true`.

### H8. Que senales usa InnerExpert y cuanto cuestan

- Modelos del paper: OLMoE-1B-7B-0924-Instruct (64 expertos, k=8) y Gemma-4-26B-A4B-it (128 + 1 compartido, k=8).
- Senales MoE: entropia del router sobre el softmax de los N expertos por capa; similitud entre expertos (promedio ponderado por routing de similitudes coseno entre salidas de expertos activados); "expert hidden score"; uso acumulado de expertos con Gini e indice de Herfindahl inverso.
- Senales estandar: score del hidden state (media de log-valores singulares de la covarianza centrada) y score de atencion.
- Usan todas las capas MoE (16 en OLMoE, 30 en Gemma).
- Detector: grilla sobre regresion logistica, random forest, XGBoost y MLP. Gana XGBoost.
- Etiquetas: juez GLM-5.1 devuelve etiqueta binaria y spans alucinados. Los spans se mapean a tokens por solapamiento de caracteres.
- Ablacion: en OLMoE domina el expert hidden score (0.693 AUROC a nivel respuesta). En Gemma domina la entropia (0.884). La combinacion llega a 0.88-0.91.
- **Costo medido por los autores: extraccion ~2.5x la generacion vainilla; total ~3x (~1.15 s por 100 tokens en OLMoE); +3% memoria GPU.**
- Implementacion: reemplazan el forward de `OlmoeSparseMoeBlock` y `Gemma4TextExperts` (monkeypatch), no usan `register_forward_hook`. Guardan `expert_hidden_states` de shape `(N, k, H)` en `self.last_experts_hidden`. No soportan Mixtral ni GptOss. Licencia MIT.

## Implicancias para Vigia

### Decision D1. Capturar la entrada del bloque MoE y recalcular los router logits

- Instalar un `register_forward_pre_hook(with_kwargs=True)` sobre cada bloque MoE (`mlp`, `block_sparse_moe` o `feed_forward`).
- El hook toma `hidden_states` `(B, T, H)` y calcula `logits = F.linear(x.float(), W.float(), b)`.
- Ventaja 1: funciona igual en v4 y v5, porque el router siempre es lineal.
- Ventaja 2: funciona con MXFP4, porque el router de gpt-oss no se cuantiza y el forward parcheado no pasa por el modulo router.
- Ventaja 3: el costo es minimo. En gpt-oss-20b son `H*E = 2880*32 ~ 92K` MACs por token y capa, contra ~4 expertos de varios millones de MACs cada uno [estimado, medir].
- Vigia nunca modifica el resultado del forward. Los hooks solo leen.

### Decision D2. Senales "gratis" (tier 1) desde los logits recalculados

Calcular por token y capa, en float32:

- Entropia normalizada del softmax sobre E: `H / log(E)`.
- Masa top-k: suma de las k probabilidades mas altas.
- Margen top-1 menos top-2, y margen entre el k-esimo y el (k+1)-esimo (cercania a la frontera de seleccion).
- Para DeepSeek-V3 y Llama4 (router sigmoide): exponer la entropia del softmax de los logits como senal derivada, y ademas la entropia de los scores sigmoides normalizados. Documentar que no es la distribucion real de routing.
- Agregados por token entre capas: media, maximo, desvio. Vector de features de tamano fijo `L x F`.
- Este tier debe cumplir la meta de latencia < 5%.

### Decision D3. Senales de desacuerdo entre expertos (tier 2, opcional)

- En v5 no hay modulos por experto. Registrar un backend propio `vigia_eager` en `ExpertsInterface`.
- El backend replica el loop eager, guarda la salida de cada experto antes de ponderar `(N, k, H)` o un resumen (coseno ponderado), y devuelve el mismo resultado. No duplica computo.
- Activarlo con `model.set_experts_implementation("vigia_eager")` y restaurar el backend previo al desactivar.
- Alternativa sin registro: un forward hook sobre `*Experts` recibe `(hidden_states, top_k_index, top_k_weights)` y recalcula cada experto. Esto duplica el computo de expertos. Usarlo solo en un subconjunto de capas.
- Guardar solo el resumen por token (coseno ponderado, norma, dispersion), nunca `(N, k, H)` completo, para no explotar memoria.
- En v4, colgar `register_forward_hook` sobre cada `experts[j]` del `ModuleList`.
- Con MXFP4 (gpt-oss en GPU), el tier 2 no esta disponible sin dequantizar. Vigia debe reportarlo como capacidad deshabilitada, no fallar.
- Marcar el tier 2 como "costoso" en la UI y medir su overhead real. InnerExpert reporta ~2.5x con su extraccion completa.

### Decision D4. Registro declarativo de adaptadores

Cada adaptador es un dato, no una clase con logica duplicada. Campos:

| Campo | Ejemplo GptOss | Ejemplo Mixtral v5 | Ejemplo GraniteMoe |
|---|---|---|---|
| `model_type` | `gpt_oss` | `mixtral` | `granitemoe` |
| `block_attr` | `mlp` | `mlp` | `block_sparse_moe` |
| `router_attr` | `router` | `gate` | `router` |
| `router_weight` | `router.weight` | `gate.weight` | `router.weight` |
| `router_bias` | `router.bias` | ninguno | ninguno |
| `routing_fn` | `topk_then_softmax` | `softmax_then_topk` (+renorm) | `topk_then_softmax` |
| `num_experts_key` | `num_local_experts` | `num_local_experts` | `num_local_experts` |
| `top_k_key` | `num_experts_per_tok` | `num_experts_per_tok` | `num_experts_per_tok` |
| `experts_attr` | `experts` | `experts` | `experts` |
| `is_moe_layer` | todas | todas | todas |

Adaptadores completos (v5):

| model_type | block_attr | router_weight (+bias) | routing_fn | E / k keys | is_moe_layer | shared expert | experts_attr |
|---|---|---|---|---|---|---|---|
| `gpt_oss` | `mlp` | `router.weight` + `router.bias` | top-k sobre logits, softmax sobre k | `num_local_experts` / `num_experts_per_tok` | todas | no | `experts` (`GptOssExperts` o `Mxfp4GptOssExperts`) |
| `mixtral` | `mlp` | `gate.weight` | softmax E, top-k, renorm | `num_local_experts` / `num_experts_per_tok` | todas | no | `experts` |
| `qwen2_moe` | `mlp` | `gate.weight` | softmax E, top-k, renorm si `norm_topk_prob` | `num_experts` / `num_experts_per_tok` | `idx not in mlp_only_layers and (idx+1) % decoder_sparse_step == 0` | `shared_expert` + `shared_expert_gate` (sigmoide, senal extra) | `experts` |
| `qwen3_moe` | `mlp` | `gate.weight` | igual a qwen2_moe | igual | igual | no | `experts` |
| `olmoe` | `mlp` | `gate.weight` | softmax E, top-k, renorm si `norm_topk_prob` | `num_experts` / `num_experts_per_tok` | todas | no | `experts` |
| `granitemoe` | `block_sparse_moe` | `router.weight` | top-k sobre logits, softmax sobre k | `num_local_experts` / `num_experts_per_tok` | todas | no | `experts` |
| `deepseek_v3` | `mlp` | `gate.weight` (+ `gate.e_score_correction_bias` para seleccion) | sigmoide, top-k por grupos, renorm, escala | `n_routed_experts` / `num_experts_per_tok` | `idx >= first_k_dense_replace` | `shared_experts` | `experts` |
| `phimoe` | `mlp` | `router.weight` (nn.Linear) | sparsemixer top-2 | `num_local_experts` / `num_experts_per_tok` | todas | no | `experts` |
| `llama4_text` | `feed_forward` | `router.weight` (nn.Linear) | top-k sobre logits, sigmoide sobre k | `num_local_experts` / `num_experts_per_tok` | `idx in config.moe_layers` | `shared_expert` | `experts` (denso) |

- Para detectar capas MoE de forma robusta, Vigia debe comprobar `isinstance` contra la clase del bloque o la existencia de `router_attr`, no solo la regla del config.
- Un adaptador desconocido debe fallar con un error claro y listar los `model_type` soportados.
- Agregar una arquitectura nueva debe ser agregar una fila (OCP por adicion).

### Decision D5. Manejo de la secuencia y del cache

- Los logits llegan aplanados `(N, E)`. El hook debe guardar `(B, T)` de la entrada y re-dar forma.
- En decodificacion con cache KV, cada paso trae T=1 (o pocos tokens). El colector debe acumular por paso y concatenar en el eje del tiempo.
- Separar prefill (prompt) de tokens generados. El detector puntua solo tokens generados por defecto.
- Con padding izquierdo en batch, aplicar el `attention_mask` para descartar posiciones de padding.
- No usar `output_router_logits=True` en `generate()`.

### Decision D6. Estrategia de tests en CPU

- Tests unitarios: un modelo diminuto aleatorio por arquitectura desde config (sin red). Matriz minima: `mixtral`, `gpt_oss`, `qwen2_moe`, `qwen3_moe`, `olmoe`, `granitemoe`, `deepseek_v3`.
- Test de oraculo 1: los logits recalculados por el pre-hook deben igualar (`torch.allclose`) a los que devuelve el router original (capturados con un hook sobre el router en backend eager).
- Test de oraculo 2: la seleccion top-k reconstruida desde los logits debe igualar `top_k_index` del router.
- Test de oraculo 3: con el backend `vigia_eager`, la salida del modelo debe ser identica a la del backend `eager`. Los hooks no alteran el output.
- Test de oraculo 4: la suma ponderada de las salidas por experto guardadas debe reproducir la salida del modulo `Experts`.
- Test de remocion: tras `detach()`, el modelo no conserva hooks (`_forward_pre_hooks` vacio) y el backend vuelve al original.
- Test de MXFP4 simulado: aplicar un parche equivalente a `mlp_forward` (que no llama al router) y verificar que el pre-hook igual captura logits.
- Test de integracion opcional (marcado `slow`, requiere descarga): `ibm-granite/granite-3.1-1b-a400m-instruct` en CPU con texto en espanol. Medir latencia con y sin Vigia.
- Fijar `torch.manual_seed` y `model.eval()`. Los routers con jitter (Mixtral, Phi-MoE) solo agregan ruido en `training`.

### Decision D7. Benchmark de latencia

- Medir el overhead en CPU con granite-1b-a400m (24 capas, 32 expertos, k=8): tier 1 contra baseline, en prefill y en decodificacion.
- Reportar mediana y p95 sobre varias corridas. La meta del plan es < 5% para tier 1.
- El overhead en GPU con gpt-oss-20b queda como dato a medir en hardware del cliente.

## Edge cases y riesgos

- **Cambio de API entre versiones.** El router de GptOss cambio su tupla de retorno entre v4.57 y v5. Granite devuelve los logits en `index=2`. Un adaptador que lee la salida del router se rompe. Mitigacion: D1 (recalcular desde la entrada).
- **MXFP4 bypasea el modulo router.** Un hook sobre `GptOssTopKRouter` no captura nada con MXFP4 activo [inferido del codigo].
- **MegaBlocks (`use_kernels=True`).** Reemplaza el forward de `GptOssMLP` desde el hub. Hay que verificar que el pre-hook del bloque sigue disparando [no verificado].
- **Kernels fusionados de expertos** (`grouped_mm`, `deepgemm`, `sonicmoe`). No exponen salidas por experto. El tier 2 requiere el backend propio o recomputo.
- **torch.compile con `fullgraph=True`.** Los hooks Python y el backend eager rompen el grafo completo. Vigia debe documentar que se usa sin compile o con compile parcial.
- **Capas densas intercaladas** (Qwen con `decoder_sparse_step`, DeepSeek con `first_k_dense_replace`, Llama4 con `moe_layers`). El numero de capas MoE varia. El vector de features debe indexar por capa MoE, no por capa total.
- **Routers sigmoides** (DeepSeek-V3, Llama4). La "entropia del router" no esta definida igual que con softmax. Las features no son comparables entre familias. El detector debe calibrarse por modelo.
- **`e_score_correction_bias` de DeepSeek** cambia la seleccion pero no los pesos. La seleccion reconstruida debe sumar el bias antes del top-k por grupos.
- **Shared experts** (Qwen2-MoE, DeepSeek, Llama4). Aportan senal no ruteada. El gate sigmoide del shared expert de Qwen2-MoE es una feature extra.
- **Precision.** Los logits en bf16 pierden resolucion. Calcular entropia en float32. DeepSeek ya calcula en float32.
- **Empates en top-k** con pesos aleatorios o cuantizados. El oraculo de seleccion debe tolerar empates exactos.
- **Memoria del colector.** Guardar `(N, k, H)` por capa en secuencias largas consume mucho. Guardar solo resumenes por token.
- **Batch con padding.** Las posiciones de padding producen logits basura. Hay que enmascararlas.
- **Hilos y reentrancia.** Si el mismo modelo sirve varias requests concurrentes, un colector global mezcla datos. El colector debe ser por request o el uso debe ser serial.
- **Fuga de hooks.** Si una excepcion corta el forward, los hooks pueden quedar instalados. Usar context manager con `finally`.
- **gpt-oss en la laptop.** No se puede probar gpt-oss-20b real en la maquina de build. Solo el modelo diminuto aleatorio y el checkpoint `tiny-random/gpt-oss-mxfp4` (dequantizado en CPU) [comportamiento en CPU no verificado].
- **Licencias.** gpt-oss y Granite son Apache 2.0. El repo InnerExpert es MIT. Vigia no debe copiar codigo de InnerExpert sin atribucion.

## Objeciones

- No ejecute ningun codigo. Todos los comportamientos de hooks son inferencias de lectura de codigo. Falta una prueba real en CPU.
- No lei el metodo `register` de `GeneralInterface`. La API exacta para registrar `vigia_eager` [no verificado].
- No lei los valores por defecto del decorador `use_experts_implementation` sin argumentos (`is_concatenated`, `is_transposed`).
- No verifique si MegaBlocks conserva el pre-hook sobre `GptOssMLP`.
- No verifique que `tiny-random/gpt-oss-mxfp4` cargue en CPU sin GPU.
- No lei el `GraniteMoeConfig` ni el `Qwen3MoeConfig` completos. Use los nombres del config.json real y del modeling.
- No verifique la estructura de GraniteMoe en v4 (`GraniteMoeTopKGating`). El adaptador v4 de Granite queda sin especificar.
- No verifique el numero de expertos activos de granite-3b-a800m ni de TinyMixtral.
- No lei las ecuaciones exactas 3-11 de InnerExpert; la conversion HTML entrego solo descripciones.
- El overhead de 2.5x de InnerExpert es en GPU con su extraccion completa. Falta medir el costo del tier 1 propuesto.
- El riesgo de vLLM (no expone router logits) queda fuera de esta nota.

## Fuentes

- Codigo v5 (main) de transformers: `modeling_mixtral.py`, `modeling_gpt_oss.py`, `modeling_qwen2_moe.py`, `modeling_qwen3_moe.py`, `modeling_olmoe.py`, `modeling_granitemoe.py`, `modeling_deepseek_v3.py`, `modeling_phimoe.py`, `modeling_llama4.py` en https://github.com/huggingface/transformers/tree/main/src/transformers/models
- Configs: `configuration_gpt_oss.py`, `configuration_mixtral.py`, `configuration_qwen2_moe.py`, `configuration_olmoe.py`, `configuration_deepseek_v3.py` (mismo repo).
- Integraciones: https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/integrations/moe.py y https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/integrations/mxfp4.py
- Captura de salidas: https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/utils/output_capturing.py
- Codigo v4.57.1: https://raw.githubusercontent.com/huggingface/transformers/v4.57.1/src/transformers/models/mixtral/modeling_mixtral.py y https://raw.githubusercontent.com/huggingface/transformers/v4.57.1/src/transformers/models/gpt_oss/modeling_gpt_oss.py
- Doc backends de expertos: https://huggingface.co/docs/transformers/experts_interface
- Doc MXFP4: https://huggingface.co/docs/transformers/quantization/mxfp4
- Blog HF sobre gpt-oss: https://huggingface.co/blog/faster-transformers
- Version PyPI: https://pypi.org/pypi/transformers/json
- gpt-oss-20b: https://huggingface.co/openai/gpt-oss-20b y su config.json y API de archivos.
- Granite: https://huggingface.co/ibm-granite/granite-3.1-1b-a400m-instruct (card, config, API), API de granite-3.1-3b-a800m-instruct.
- OLMoE: https://huggingface.co/api/models/allenai/OLMoE-1B-7B-0125-Instruct?blobs=true
- Qwen1.5-MoE: https://huggingface.co/api/models/Qwen/Qwen1.5-MoE-A2.7B?blobs=true
- TinyMixtral: https://huggingface.co/api/models/Isotonic/TinyMixtral-4x248M-MoE?blobs=true
- Modelos aleatorios: https://huggingface.co/api/models/tiny-random/gpt-oss-bf16?blobs=true y busquedas del hub `tiny-random-gpt-oss`, `tiny-random-Mixtral`, `TinyMixtral`.
- Foro HF sobre router logits en generate: https://discuss.huggingface.co/t/evalutation-of-expert-router-logits-simultanous-to-generation/170612
- Paper InnerExpert: https://arxiv.org/html/2608.17687
- Repo InnerExpert: https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
