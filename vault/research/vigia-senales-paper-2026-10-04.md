---
title: Vigia - senales por token del paper InnerExpert y literatura de incertidumbre
mision: Que features por token puede calcular Vigia sobre un MoE, con que formula exacta, a que costo, y contra que baseline se mide.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://arxiv.org/pdf/2608.17687
  - https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/metrics/_metrics.py
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/monitoring/_llm_monitor.py
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/forwards/_model_forwards.py
  - https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/experiments/0.2-compute-monitor-metrics.py
  - https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
  - https://huggingface.co/openai/gpt-oss-20b/blob/main/config.json
  - https://huggingface.co/openai/gpt-oss-120b/blob/main/config.json
  - https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
  - https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
  - https://arxiv.org/html/2402.03744
  - https://pmc.ncbi.nlm.nih.gov/articles/PMC11186750/
  - https://arxiv.org/abs/2406.15927
  - https://arxiv.org/abs/2304.13734
  - https://arxiv.org/abs/2409.17504
  - https://arxiv.org/html/2509.03531v2
  - https://arxiv.org/abs/2608.28930
  - https://arxiv.org/abs/2602.09158
  - https://arxiv.org/html/2509.04816v1
  - https://arxiv.org/abs/2607.26052
  - https://arxiv.org/abs/2509.23830
  - https://portal.odesia.uned.es/en/dataset/mu-shroom-2025-es
  - https://arxiv.org/abs/2504.11975
created: 2026-10-04
---

# Vigia - senales por token (paper InnerExpert + literatura)

## Tronco

El paper InnerExpert existe y es reproducible, pero mide solo OLMoE y Gemma-4 en ingles, cuesta ~3x la latencia vanilla con todas las senales, y ninguna senal MoE sola es estable: Vigia necesita features por niveles de costo, calibracion por modelo y cliente, y la entropia de salida como baseline obligatorio.

## Hallazgos

### 1. Identidad del paper (verificado)

- Titulo: "Mixture-of-Expert Blocks Contain Strong Hallucination Detection Signals".
- Autores: Joao Fonseca, Rodrigo Rodrigues, Paolo Romano (INESC-ID, Instituto Superior Tecnico, Lisboa).
- arXiv: 2608.17687v1, 18 ago 2026. El PDF trae copyright AAAI 2027.
- Codigo: github.com/joaopfonseca/InnerExpert-Hallucination-Detection, licencia MIT.
- Libreria interna: `moeuncert` (modulos `forwards`, `metrics`, `monitoring`, `baselines`).
- Leí el PDF completo (20 paginas) y cuatro archivos fuente del repo.

### 2. Notacion base del paper

- Prompt `x = (x_1..x_U)`. Respuesta `y = (y_1..y_T)`. Muestreo `p_θ(y_t | x, y_<t)`.
- `N` expertos por capa. `k` expertos activos. `L` capas. `H` cabezas de atencion. `d` dimension oculta.
- `h_{t,l} ∈ R^d` es el estado oculto que entra a la capa `l`.
- `h'_{t,l}` es la salida de self-attention mas post-attention layernorm. Es la entrada del bloque MoE.
- Router: `g_l(h'_{t,l}) = softmax(W^g_l h'_{t,l}) ∈ R^N` (Eq. 1).
- `S_{t,l}` es el conjunto top-k elegido.
- Salida MoE: `MoE_l(h') = Σ_{i∈S} g_{l,i}(h') E_i(h')`. Residual: `h_{t,l+1} = h_{t,l} + MoE_l(h')` (Eq. 2).
- Salida de experto: `e_{t,l,i} = E_i(h'_{t,l})`.
- Todas las senales se calculan "acumuladas sobre el prefijo generado y_≤t". Solo sobre tokens generados, no sobre el prompt.

### 3. Las ocho senales de InnerExpert (formulas exactas del paper)

| Senal | Formula (paper) | Categoria | Dim por token | Insumo |
|---|---|---|---|---|
| Hidden state score | `Σ_{t,l} = H_{t,l}^T J H_{t,l} + αI`, `J = I − (1/d)11^T`, `α=0.001`; `φ^hid_{t,l} = (1/t) Σ_{j=1..t} log σ_j(Σ_{t,l})` (Eq. 3) | Estandar (LLM-Check) | L | hidden states |
| Attention score | `φ^att_{t,l,h} = Σ_{j=1..t} log(A_{t,l,h}[j,j] + ε)` (Eq. 4) | Estandar (LLM-Check) | L·H | matrices de atencion |
| Router entropy | `φ^rout_{t,l} = −Σ_{i=1..N} g_{l,i} log g_{l,i}` (Eq. 5) | MoE | L | router logits |
| Expert hidden score | `φ^exp-hid_{t,l} = Σ_{i∈S} g_{l,i} φ^hid_{t,l,i}`, con φ^hid sobre la secuencia `{e_{1,l,i}..e_{t,l,i}}` (Eq. 6) | MoE | L | salidas por experto |
| Expert similarity | `φ^sim_{t,l} = Σ_{i∈S} Σ_{j∈S} g_{l,i} g_{l,j} cos(e_{t,l,i}, e_{t,l,j})` (Eq. 7) | MoE | L | salidas por experto |
| Expert usage | `c_{t',l,i} = g_{l,i}` si `i∈S_{t',l}`, si no 0; `u_{t,l} = Σ_{t'≤t} c_{t',l} / ‖Σ_{t'≤t} c_{t',l}‖_1 ∈ Δ^{N−1}` (Eqs. 8-9) | MoE | L·N | router (pesos top-k) |
| Gini impurity | `φ^gini_{t,l} = 1 − Σ_i u_{t,l,i}^2` (Eq. 10) | MoE | L | usage |
| Inverse Herfindahl | `φ^herf_{t,l} = (Σ_i u_{t,l,i}^2)^{-1}` (Eq. 11) | MoE | L | usage |

- Vector final: `Φ_t = {φ^hid, φ^att_{1..H}, φ^rout, φ^exp-hid, φ^sim, φ^gini, φ^herf, u}` por capa, aplanado (Eq. 12).
- Dimension total: `5L + LH + LN` (Tabla 4).
- OLMoE-1B-7B (L=16, H=16, N=64): `|Φ_t| = 1360`. Gemma-4-26B-A4B (L=30, H=16, N=129): `|Φ_t| = 4470` (Tabla 5).
- Hay una novena senal que aparece en tablas pero no en ecuaciones: "Exp. Entropy" o "Usage Entropy". El codigo la calcula como entropia de `u_{t,l}`: `topk_entropy(expert_usage, softmax=False)`.
- Normalizacion: z-score a todo excepto `u` (ya acotado en [0,1]).
- Capas: todas. Cabezas: todas. No hay seleccion de capas.

### 4. Discrepancias paper vs codigo (verificado leyendo el repo)

Estas diferencias importan para reproducir los numeros.

1. **Router entropy sobre top-k, no sobre N.** El paper dice entropia sobre los N expertos (Eq. 5). El codigo llama `topk_entropy(outputs["expert_weights"], softmax=False)`. `expert_weights` son los pesos de los k expertos elegidos (`_model_forwards.py` guarda `expert_idx`, `expert_weights`, `expert_hidden_states` con forma `(batch, seq, top_k, hidden)`). En OLMoE esos pesos no suman 1 si `norm_topk_prob=False` [no verificado el valor del flag en ese config]. Entonces la "entropia" del codigo es la de un vector parcial.
2. **Hidden score no causal.** El codigo arma `Sigma = H J H^T` sobre la secuencia completa (matriz `seq_len × seq_len`). Luego hace `cumsum(log svdvals) / arange(1..t)`. El valor en la posicion t usa los t mayores valores singulares de la secuencia ENTERA. Eso filtra informacion de tokens futuros. No sirve tal cual para streaming.
3. **Eq. 3 tiene dimensiones inconsistentes.** `H^T J H` con `H ∈ R^{t×d}` y `J ∈ R^{d×d}` no cierra. El codigo resuelve con `H J H^T` (Gram t×t, centrado sobre la dimension oculta). El promedio `(1/t) Σ_{j=1..t}` confirma la lectura Gram.
4. **"Per-expert" en codigo es por slot top-k, no por identidad de experto.** `expert_hidden_score` permuta la dimension `top_k` como si fuera "experto". La secuencia del "slot 1" mezcla expertos distintos en cada token. El paper describe secuencias por experto. Interpretacion mia del codigo; confirmar corriendo.
5. **Bug de dimension en usage.** `one_hot(expert_idx, num_classes=expert_idx.max()+1)`. Si el experto de indice mas alto nunca se usa, el vector de usage es mas corto que N. Gini no cambia. Usage entropy y la dimension de `u` si cambian entre respuestas.
6. **Logit entropy del baseline usa top-5.** `topk_entropy(outputs["scores"], k=5)`: softmax completo, luego toma top-5 sin renormalizar. No es la entropia de vocabulario completo.
7. **Hiperparametros.** El texto dice cinco familias de clasificador y despues enumera cuatro en una frase. Tabla 7 lista cinco (LR, RF, XGBoost, MLP, Transformer).

### 5. Extraccion (como se obtiene cada insumo)

- Router logits: flag `output_router_logits=True` de HuggingFace en `generate()`.
- Atencion: flag `output_attentions=True`. Esto exige atencion eager en la practica (SDPA y flash no devuelven pesos) [no verificado que el repo fuerce eager].
- Salidas por experto: monkey-patch del `forward` de cada bloque MoE (`OlmoeSparseMoeBlock`, `Gemma4TextExperts`). Guarda las salidas pre-ponderacion en `last_experts_hidden`. Un forward hook lo recoge en cada paso.
- Generacion incremental con KV cache (`use_cache=True`).
- Agregar un modelo nuevo exige escribir `forward_<model>` y registrarlo en `MOE_FORWARD_REGISTRY`.
- Memoria extra: OLMoE 12.7 → 13.1 GB (~3%). Gemma 46.8 → 48.1 GB (2.8%). El costo dominante es el tensor `(B, S, k, d)` por capa por paso.
- Disco del proyecto: ~1.7 TB de archivos crudos. Detectores entrenados: 311 MB.

### 6. Etiquetado con juez LLM (verificado)

- Fuente de preguntas: RealTimeQA, ene 2024 a dic 2025 (train). Test temporal OOD: ene-jun 2026, 365 preguntas.
- Por pregunta, el host genera dos respuestas: sin evidencia (base) y con evidencia (simula RAG).
- Decodificacion greedy, maximo 65 tokens nuevos.
- Juez: GLM-5.1 via DeepInfra (`zai-org/GLM-5.1`).
- Salida del juez: JSON `{"label": 0|1, "hallucinated_spans": ["substring exacto", ...]}`. Si label=0, la lista es vacia.
- Etiqueta por token: `ℓ_t = 1` si el span de caracteres del token se solapa con algun span alucinado.
- Split train/val estratificado por tasa de alucinacion y agrupado por id de pregunta (evita fuga).
- Umbral τ: maximiza F1 con barrido de curva PR.
- Validacion humana: 200 respuestas, un solo anotador, muestreo estratificado 2×2 (heuristica debil × juez).
  - Juez vs humano: 86.5% de acuerdo (27 errores). 20 falsos "alucinado". 7 falsos "fundado".
  - Heuristica BLEU/ROUGE/BERTScore vs humano: 59.5%.
- Desbalance: OLMoE 83.0% alucinado (99.7% base, 66.2% con evidencia). Gemma 49.4% (85.4% base, 13.3% con evidencia).

### 7. Datasets y modelos host

| Item | Valor |
|---|---|
| Train | RealTimeQA 2024-2025 (4636 respuestas por host, Tabla 11) |
| Test temporal | RealTimeQA ene-jun 2026, 365 preguntas |
| Test cruzado | SQuAD, TruthfulQA, NQ-Open, FreshQA; 200 preguntas cada uno |
| Idioma | Solo ingles. Multilingue queda como trabajo futuro |
| Host 1 | OLMoE-1B-7B-0924-Instruct: 64 expertos, 8 activos |
| Host 2 | Gemma-4-26B-A4B-it: 128 expertos + 1 compartido, 8 activos |
| No probados | gpt-oss, Mixtral, Qwen-MoE, DeepSeek, Granite MoE |
| Hardware | 2× RTX PRO 6000 Blackwell (96 GB c/u), EPYC 64 nucleos, 258 GiB RAM |

### 8. AUROC por token (Tabla 1, verificado)

| Metodo | Gemma-4 avg | OLMoE avg |
|---|---|---|
| IE (XGBoost) | **0.753** | 0.736 |
| IE (LR) | 0.725 | **0.762** |
| IE (MLP) | 0.730 | 0.754 |
| IE (RF) | 0.710 | 0.594 |
| IE (Transformer) | 0.694 | 0.619 |
| LLM-Check hidden | 0.551 | 0.605 |
| Logit entropy (baseline) | 0.541 | 0.639 |
| LLM-Check attention | 0.313 | 0.303 |

- Mejora sobre el mejor baseline por token: +0.12 (OLMoE) y +0.20 (Gemma).
- El mejor por-dataset es RealTimeQA OOD temporal: 0.830 (OLMoE LR), 0.805 (Gemma XGB).
- LLM-Check attention por token queda bajo 0.5 en ambos modelos. Esta anticorrelacionada.

### 9. AUROC por respuesta (Tabla 2, verificado)

| Metodo | Gemma-4 avg | OLMoE avg |
|---|---|---|
| IE (XGBoost) | **0.912** | **0.882** |
| HaluNet (reimpl.) | 0.901 | 0.851 |
| IE (MLP) | 0.896 | 0.874 |
| LLM-Check hidden | 0.722 | 0.817 |
| SelfCheckGPT NLI | 0.707 | 0.784 |
| Semantic Energy | 0.475 | 0.866 |
| Semantic Uncertainty | 0.584 | 0.770 |
| Perplexity | 0.608 | 0.172 |
| Logit entropy | 0.604 | 0.381 |

- Agregacion respuesta: `S(y) = max_t s(Φ_t)` (Eq. 13).
- Los metodos de muestreo usan K=5, temperatura 0.7, top-p 0.9.
- Semantic Energy es inestable: 0.866 en OLMoE y 0.475 en Gemma.

### 10. Senales individuales (Tabla 3, verificado)

| Senal | OLMoE resp. | OLMoE token | Gemma resp. | Gemma token |
|---|---|---|---|---|
| Inv. Herfindahl | 0.685 | **0.675** | 0.784 | 0.602 |
| Usage entropy | 0.613 | 0.574 | **0.884** | 0.591 |
| Usage Gini | 0.599 | 0.561 | 0.824 | **0.626** |
| Expert hidden | 0.693 | 0.640 | 0.722 | 0.466 |
| Expert similarity | 0.397 | 0.442 | 0.717 | 0.437 |
| Router entropy | 0.565 | 0.574 | 0.327 | 0.446 |
| Hidden score | **0.817** | 0.605 | 0.722 | 0.551 |
| Attention score | 0.748 | 0.303 | 0.799 | 0.313 |
| Logit entropy | 0.381 | 0.639 | 0.604 | 0.541 |

- Ninguna senal MoE sola es fuerte en los dos modelos.
- Router entropy esta ANTICORRELACIONADA en Gemma (0.327 respuesta, 0.446 token).
- Expert similarity es casi azar en OLMoE.
- Las senales de distribucion de uso (Herfindahl, Gini, usage entropy) son las MoE mas utiles por token.
- La combinacion supera a cualquier senal sola. La ganancia relativa es mayor por token.

### 11. F1 y sensibilidad del umbral (Tablas 12, 13, 15)

- IE (XGBoost) colapsa en F1 por token en Gemma: 0.276 promedio (0.116 en NQ-Open). El AUROC era el mejor.
- IE (LR) y IE (MLP) son los mas estables en F1. Token F1: LR 0.693 (Gemma), MLP 0.774 (OLMoE).
- Causa: el umbral optimizado en train no transfiere entre datasets.
- En OLMoE el F1 de respuesta esta inflado por el desbalance (casi todo es alucinado).

### 12. Latencia (Tabla 14, verificado)

| Metodo | OLMoE s/100 tok | Gemma s/100 tok | Overhead OLMoE |
|---|---|---|---|
| Vanilla | 1.146 | 3.825 | 0% |
| Logit entropy | 1.152 | 3.830 | +0.5% |
| LLM-Check attention | 1.198 | 3.965 | +4.5% |
| LLM-Check hidden | 1.246 | 4.133 | +8.7% |
| HaluNet | 1.257 | 4.128 | +9.7% |
| Cualquier senal MoE (extraccion conjunta) | 2.829 | 7.677 | +147% |
| IE completo (LR) | 3.493 | 9.376 | +205% |
| SelfCheckGPT NLI (K=5) | 5.966 | 19.331 | +421% |

- Todas las senales MoE comparten el mismo tiempo porque se extraen juntas. El paper no aisla el costo de router entropy sola.
- El overhead de ~3x choca con la meta de Vigia (< 5%).
- La medicion fue sobre 50 preguntas, en GPU, con HF transformers (no vLLM).

### 13. gpt-oss (modelo objetivo de Vigia) - arquitectura relevante (verificado)

| Parametro | gpt-oss-20b | gpt-oss-120b |
|---|---|---|
| Capas L | 24 | 36 |
| Expertos N | 32 | 128 |
| Activos k | 4 | 4 |
| hidden d | 2880 | 2880 |
| Cabezas H | 64 | 64 |
| KV heads | [no verificado] | 8 |
| Atencion | alterna sliding (ventana 128) y full | igual |
| Cuantizacion | MXFP4 en expertos; router, atencion, embeddings y lm_head fuera de MXFP4 | igual |
| `output_router_logits` default | false | [no verificado] |

- Router en HF (`GptOssTopKRouter`): `router_logits = linear(h, W, b)`; luego `topk(router_logits, k)`; luego `softmax` SOLO sobre los k valores top.
- Consecuencia: gpt-oss nunca calcula un softmax sobre los N expertos. La entropia "sobre N" de Eq. 5 es una cantidad derivada que Vigia debe computar aparte. La entropia "sobre k" es la que el modelo usa.
- Los router logits completos (N valores) se capturan con `OutputRecorder(GptOssTopKRouter, index=0)`.
- `GptOssExperts` materializa `out` por experto antes de `index_add_`. Un patch puede guardar `out` sin ponderar. Hay rutas de kernel MXFP4/Triton que pueden no pasar por ese loop [no verificado].
- Atencion con "sinks": un logit aprendido entra al softmax y luego se descarta. Las filas de atencion no suman 1. Esto altera la Attention score de LLM-Check.
- Dimension de `Φ_t` estilo InnerExpert: 20b = 5·24 + 24·64 + 24·32 = **2424**. 120b = 5·36 + 36·64 + 36·128 = **7092**.

### 14. vLLM (riesgo tecnico del plan, verificado)

- vLLM tiene `--enable-return-routed-experts` ("routed experts replay").
- Devuelve SOLO ids de experto (int16), no pesos ni logits.
- Formas: `prompt_routed_experts [prompt_len, num_moe_layers, top_k]` y `routed_experts [gen_len, num_moe_layers, top_k]`.
- Los datos llegan solo al terminar el request. No hay streaming incremental.
- Solo engine V1. Sin async scheduling. Un request preemptado pierde los datos sin aviso.
- Overhead reportado: ~2% de throughput, ~14 MB por GPU.
- Router logits por capa: vLLM no los expone de forma nativa (hilo del foro de vLLM). Hace falta modificar el codigo del modelo.
- Logprobs de salida: vLLM devuelve top-k logprobs por token (usualmente hasta 20) [no verificado el maximo en la version actual].

### 15. Literatura de incertidumbre por token y por respuesta

| Metodo | Nivel | Pasadas | Insumo | Formula / idea | Numero reportado | Fuente |
|---|---|---|---|---|---|---|
| Entropia de salida / logprob | token | 1 | logits | `H_t = −Σ_v p_t(v) log p_t(v)`; `NLL_t = −log p_t(y_t)` | baseline en todos | varios |
| Semantic Entropy (Farquhar 2024, Nature) | respuesta | ~10 | muestras + NLI | `SE(x) = −Σ_c P(c|x) log P(c|x)`, `P(c|x) = Σ_{s∈c} P(s|x)` (normalizado por longitud); cluster por entailment bidireccional | AUROC 0.790 vs 0.691 entropia ingenua | PMC11186750 |
| Discrete SE | respuesta | ~10 | solo texto | `P(c|x)` = fraccion de muestras en el cluster | — | idem |
| Semantic Entropy Probes (Kossen, Han et al. 2024) | respuesta | 1 | hidden states | probe lineal predice SE binarizada; posiciones TBG y SLT | overhead casi cero; generaliza mejor OOD que accuracy probes | 2406.15927 |
| SAPLMA (Azaria y Mitchell 2023) | enunciado | 1 | activaciones ocultas | clasificador sobre activaciones | 71-83% accuracy | 2304.13734 |
| INSIDE / EigenScore (Chen 2024, ICLR) | respuesta | K=10 | embedding ultimo token, capa L/2 | `Σ = Z^T J_d Z`; `E = (1/K) log det(Σ + αI_K) = (1/K) Σ log λ_i`, α=0.001; feature clipping p=0.2% | AUROC 80.4-83.9 (LLaMA 7B/13B) | 2402.03744 |
| HaloScope (Du, Xiao, Li 2024, NeurIPS) | respuesta | 1 en test | hidden states, datos sin etiqueta | score de pertenencia a subespacio (SVD) + clasificador binario | "supera por margen" [numeros no verificados] | 2409.17504 |
| LLM-Check (Sriramanan 2024, NeurIPS) | respuesta (adaptado a token por IE) | 1 | hidden, atencion, logits | log-det de Gram de hidden; suma de log diag de atencion | ver tablas IE | NeurIPS 2024 |
| Probes de entidades (Obeso et al. 2025) | token / entidad | 1 | residual stream capa ⌊0.95L⌋ | `p_i = σ(w^T h_i^(ℓ) + b)`; entidad = `max_{i∈s} p_i`; variante LoRA | AUC 0.854/0.867 lineal, 0.894/0.905 LoRA (Llama 8B/70B); entropia token 0.742; SE 0.719 | 2509.03531 |
| Mean shift (Lee, Seo, Lim 2026) | respuesta | 1 | hidden states | LR L2 sobre hidden basta | AUROC 0.952 | 2608.28930 |
| Metricas geometricas (Yeats et al.) | respuesta | 1 | hidden | sensibles al dominio; normalizacion por dominio | +34 puntos AUROC multi-dominio | 2602.09158 |
| Pavlitska et al. 2025 (vision) | pixel | 1 | MoE | entropia predictiva, informacion mutua, varianza entre expertos, entropia del gate | MoE mejor calibrado que ensembles bajo OOD | 2509.04816 |
| CARE (2026) | token | 1 | router MoE-LoRA | concentracion del router + desacuerdo entre expertos | AUROC 0.668 [no verificado: no aparece en el abstract leido] | 2607.26052 |
| Bayesian MoE (Li 2025) | token | 1 | router | distribucion sobre la decision de ruteo; requiere modificar el router | mejora calibracion y OOD [sin numeros en abstract] | 2509.23830 |

- Descomposicion teorica (Apendice A del paper): `H[p(y_t)] = E_θ[H[p(y_t|θ)]] + I(y_t; θ)`. Total = aleatoria + epistemica.
- Argumento: la entropia de salida mide sobre todo incertidumbre aleatoria. El desacuerdo entre expertos aproxima la epistemica, analogo a desacuerdo de ensemble.
- El paper reconoce que la conexion es empirica. Un LLM mal especificado no tiene posterior `p(θ)`.

### 16. Datos en espanol (para la brecha de idioma)

- Mu-SHROOM (SemEval-2025 Task 3): deteccion de spans alucinados en 14 idiomas, incluido espanol.
- Split espanol: 200 unidades (50 dev, 150 test), licencia CC-BY-4.0 (portal ODESIA, UNED).
- Mejores sistemas en espanol: IoU ~0.51-0.53 (ATLANTIS 0.53).
- Mu-SHROOM trae salidas de otros modelos. No trae estados internos de gpt-oss. Sirve para validar al juez en espanol, no para entrenar el detector directamente.

## Implicancias para Vigia

### A. Lista canonica de features por token

Notacion: `z_{t,l} ∈ R^N` son los router logits completos. `p_{t,l} = softmax(z_{t,l})` es la distribucion completa sobre N. `S_{t,l}` es el top-k. `w_{t,l,i}` son los pesos que el modelo usa realmente (en gpt-oss: softmax sobre los k logits top). `e_{t,l,i}` es la salida del experto i sin ponderar. `q_t ∈ Δ^{V}` es la distribucion de salida.

**Nivel 0 - solo logits de salida (baseline obligatorio, cualquier modelo, compatible con vLLM)**

| ID | Feature | Formula | Costo por token |
|---|---|---|---|
| F0.1 | Entropia de salida completa | `H_t = −Σ_v q_t(v) log q_t(v)` | O(V); V ~201k en gpt-oss [no verificado] |
| F0.2 | NLL del token emitido | `−log q_t(y_t)` | O(1) dado softmax |
| F0.3 | Entropia top-k de salida | `−Σ_{v∈top-k} q̃ log q̃`, `q̃` renormalizado; k=20 para vLLM | O(k) |
| F0.4 | Margen top1-top2 | `q_t(v_1) − q_t(v_2)` | O(V) o O(k) |

**Nivel 1 - solo router logits (sin salidas de expertos)**

| ID | Feature | Formula | Costo por token |
|---|---|---|---|
| F1.1 | Entropia del router sobre N | `−Σ_{i=1..N} p_{t,l,i} log p_{t,l,i}` | O(L·N) |
| F1.2 | Entropia del router sobre k (la que usa el modelo) | `−Σ_{i∈S} w_{t,l,i} log w_{t,l,i}` | O(L·k) |
| F1.3 | Masa top-k | `Σ_{i∈S} p_{t,l,i}` | O(L·N) |
| F1.4 | Margen de frontera del ruteo [propuesta Vigia, no esta en el paper] | `z_{(k)} − z_{(k+1)}` (logit k-esimo menos el siguiente) | O(L·N log k) |
| F1.5 | Usage ponderado acumulado | `u_{t,l}` (Eqs. 8-9), con N fijo | O(L·N) incremental |
| F1.6 | Gini de usage | `1 − Σ_i u_{t,l,i}^2` | O(L·N) |
| F1.7 | Inverse Herfindahl | `(Σ_i u_{t,l,i}^2)^{-1}` | O(L·N) |
| F1.8 | Entropia de usage | `−Σ_i u_{t,l,i} log u_{t,l,i}` | O(L·N) |
| F1.9 | Tasa de cambio de ruta [propuesta Vigia] | `1 − |S_{t,l} ∩ S_{t−1,l}| / k` | O(L·k) |

**Nivel 1b - solo ids de experto (vLLM routed experts, post-hoc)**

| ID | Feature | Formula | Nota |
|---|---|---|---|
| F1b.1-3 | Gini, Herfindahl, entropia de usage SIN ponderar | igual a F1.6-F1.8 con `c_{t',l,i} = 1[i∈S]` | solo al final del request |
| F1b.4 | Tasa de cambio de ruta | igual a F1.9 | idem |

**Nivel 2 - salidas por experto (requiere patch del forward de expertos)**

| ID | Feature | Formula | Costo por token |
|---|---|---|---|
| F2.1 | Expert similarity | `Σ_{i,j∈S} w_i w_j cos(e_i, e_j)` (incluye i=j) | O(L·k²·d); 20b: 24·16·2880 ≈ 1.1 M FLOP |
| F2.2 | Expert similarity sin diagonal [propuesta Vigia] | `Σ_{i≠j} w_i w_j cos(e_i,e_j) / Σ_{i≠j} w_i w_j` | idem |
| F2.3 | Dispersion ponderada entre expertos [propuesta Vigia, analoga a "expert variance" de Pavlitska] | `Σ_{i∈S} w_i ‖e_i − ē‖² / ‖ē‖²`, `ē = Σ w_i e_i` | O(L·k·d) |
| F2.4 | Varianza de normas de expertos [propuesta Vigia] | `Var_{i∈S}(‖e_i‖)` | O(L·k·d) |

**Nivel 3 - hidden states y atencion (caro o requiere eager)**

| ID | Feature | Formula | Costo por token |
|---|---|---|---|
| F3.1 | Probe lineal en residual | `σ(w^T h_{t,ℓ} + b)`, ℓ ≈ ⌊0.95L⌋ (Obeso) o barrido | O(d) = 2880 MAC. Barato y fuerte |
| F3.2 | Hidden score causal en ventana | Gram `G = H_W J H_W^T + αI` sobre los ultimos W tokens; `(1/W) Σ log σ_j(G)` | O(L·W²·d + L·W³) por token |
| F3.3 | Expert hidden score | Eq. 6 con F3.2 por slot o por experto | k veces F3.2 |
| F3.4 | Attention diag | `log A_{t,l,h}[t,t]`, acumulado | requiere logits q_t·k_t y logsumexp por cabeza; con eager todo se encarece |

- Lo que pide solo router logits: F1.x. Lo que pide salidas de expertos: F2.x y F3.3. Lo que pide hidden states: F3.1 y F3.2. Lo que pide atencion: F3.4.
- La "dispersion entre expertos" del plan de negocio corresponde a F2.1-F2.3. No es gratis: exige interceptar las salidas por experto.

### B. Decisiones de diseno recomendadas

1. **Baseline obligatorio = F0.1 (entropia de salida completa) y F0.2.** Reportar siempre ambos al lado del detector. Reportar tambien F0.3 con k=20 porque es lo unico disponible en vLLM.
2. **Agregar un probe lineal F3.1 como segundo baseline fuerte.** InnerExpert no lo compara. Obeso et al. reportan AUC 0.85-0.90 con un probe lineal por token. Si Vigia no le gana a un probe lineal, la tesis "senales MoE" no se sostiene.
3. **Arquitectura por niveles con presupuesto de latencia.** Nivel 0+1 es el modo por defecto ("meta < 5%"). Nivel 2 es opcional. Nivel 3 queda solo para modo offline o auditoria.
4. **Implementar F1.1 y F1.2 por separado.** El paper y su codigo usan definiciones distintas. En gpt-oss la entropia sobre k es la que el modelo usa.
5. **Fijar N en el vector de usage.** No inferir N del maximo indice visto (bug del repo).
6. **Todas las features deben ser causales.** El score del token t solo usa tokens ≤ t. Test explicito: el score de t no cambia al agregar tokens despues.
7. **Calibracion por (modelo, cliente).** Router entropy cambia de signo entre modelos. El detector debe aprender pesos por modelo. Esto encaja con "calibracion por cliente" como producto pago.
8. **Clasificador por defecto: regresion logistica.** Es la mas estable en F1 y la mejor en OLMoE por token. Exportable a JSON (pesos + medias + desvios) y corre en CPU sin dependencias. XGBoost y MLP quedan como opcionales.
9. **Calibrar probabilidades y umbral por dataset del cliente.** Usar Platt o isotonica sobre un set de validacion del cliente. Recalcular τ por cliente. El colapso de XGBoost en F1 prueba que τ no transfiere.
10. **Reportar AUROC por token, AUROC por respuesta (max), F1 a τ, y IoU de spans.** IoU permite comparar con Mu-SHROOM.
11. **Medir latencia por nivel, no en bloque.** El paper midio todas las senales MoE juntas. Vigia debe reportar overhead de F0, F1, F2 y F3 por separado.
12. **Integracion vLLM en dos modos.** Modo A "post-hoc": ids de experto (F1b) + top-20 logprobs (F0.3), sin patch. Modo B "tiempo real": plugin o fork que capture router logits por capa. El Modo A es entregable sin tocar vLLM.
13. **Juez de calibracion con salida JSON de spans exactos.** Replicar el esquema del paper (`label`, `hallucinated_spans`). Validar que cada span sea substring exacto. Mapear a tokens por solapamiento de offsets de caracteres.
14. **Validar el juez en espanol antes de entrenar.** Usar Mu-SHROOM ES (150 test) y una muestra humana del cliente. Exigir acuerdo ≥ 86.5% (lo que el paper midio en ingles) antes de confiar en las etiquetas.
15. **Generar datos de calibracion en dos condiciones.** Sin evidencia y con evidencia, como el paper. Esto da diversidad de etiquetas y simula el RAG del cliente.
16. **Split agrupado por id de pregunta.** Evita fuga entre las dos respuestas de la misma pregunta.
17. **Tests en CPU con modelos MoE diminutos.** Construir configs tiny de `GptOss`, `Mixtral`, `OlmoE` con pesos aleatorios en transformers (sin descargas). Validar formulas con casos cerrados:
    - Router uniforme → F1.1 = log N, F1.7 = N, F1.6 = 1 − 1/N.
    - Router one-hot → F1.1 = 0, F1.7 = 1.
    - Expertos identicos → F2.1 = (Σ w)² = 1 con pesos normalizados; F2.3 = 0.
18. **Formato de salida por token estable.** Por cada token: id, texto, offsets de caracteres, features crudas por nivel, score calibrado, flag sobre τ. El panel y la accion por fragmento (revision humana, re-consulta RAG, abstencion) consumen este formato.
19. **Agregacion a spans.** Agrupar tokens contiguos sobre τ en un fragmento. Score de fragmento = max de sus tokens (como Obeso y como Eq. 13).
20. **Registrar version de modelo, cuantizacion y kernel.** MXFP4 vs bf16 o un kernel distinto pueden cambiar el ruteo. La calibracion queda atada a esa tupla.

### C. Costo relativo estimado para gpt-oss-20b

- Forward por token: ~2 × 3.6 B parametros activos ≈ 7 GFLOP [parametros activos de gpt-oss-20b no verificados en esta nota].
- Nivel 1 completo: O(L·N) = 24·32 = 768 operaciones por feature. Despreciable frente al forward. El costo real es copiar logits de GPU a CPU por capa.
- Nivel 2 (F2.1): ~1.1 MFLOP por token. Despreciable en FLOPs. El costo real es memoria (k·d por capa) y romper kernels fusionados.
- F3.1: 2880 MAC. Despreciable.
- F3.2: con W=64, por capa ~W²·d + W³ ≈ 12 M + 0.26 M. Por 24 capas ≈ 300 MFLOP por token. ~4% de un forward en FLOPs, pero SVD en serie es lenta.
- Estas son estimaciones analiticas. Vigia debe medirlas. El paper solo midio el bloque completo (+147% a +205%).

## Edge cases y riesgos

- **Transferencia de modelo no probada.** El paper no midio gpt-oss. gpt-oss usa softmax sobre top-k y 4 activos (OLMoE y Gemma usan 8). Router entropy podria comportarse distinto.
- **Signo inestable.** Router entropy anticorrelacionada en Gemma. Un detector con signo fijo falla al cambiar de modelo.
- **Idioma.** Todo el paper es ingles. Los tokens del espanol se segmentan distinto y el ruteo cambia con el idioma [hipotesis, no verificada].
- **Ruido de etiquetas.** 13.5% de desacuerdo juez-humano, sesgado a falsos "alucinado" (20 de 27). El detector aprende parte del sesgo del juez.
- **Desbalance de clases.** Con 83% alucinado, F1 y accuracy enganan. Reportar AUROC y PR-AUC.
- **Umbral no transferible.** τ optimizado en un dataset colapsa en otro (XGBoost Gemma: token F1 0.276).
- **Fuga de futuro.** El hidden score del repo usa la secuencia completa. Un detector entrenado con eso parece mejor offline y peor en streaming.
- **Senales acumuladas dependen de la longitud.** Usage, Gini y Herfindahl evolucionan con t. Tokens tempranos tienen menos contexto. Respuestas largas pueden desplazar la distribucion (el paper limita a 65 tokens).
- **Respuestas largas fuera de rango.** Entrenar con respuestas cortas y evaluar largas falla (Obeso: short-to-long pierde ~0.10 AUC).
- **Dominio.** Las metricas geometricas son sensibles al dominio (Yeats et al.). Finanzas, salud y legal necesitan calibracion propia.
- **Atencion con sinks y ventana.** En gpt-oss la fila de atencion no suma 1 y las capas sliding tienen ventana 128. La Attention score cambia de significado.
- **Kernels fusionados.** vLLM y kernels MXFP4/Triton no materializan salidas por experto. Los niveles 2 y 3 pueden ser imposibles sin perder velocidad.
- **vLLM preemption.** Un request preemptado pierde los ids de experto sin aviso. Vigia debe tratar "sin datos de ruteo" como estado explicito, no como confianza alta.
- **Expertos compartidos.** Gemma-4, DeepSeek y Qwen-MoE tienen expertos siempre activos. Incluirlos en usage o entropia distorsiona las senales. Excluirlos por defecto y documentarlo.
- **Tokens del prompt.** Las senales se calculan solo sobre tokens generados. La pasada de prefill no aporta features.
- **Batching.** Con batch >1 y padding, el usage acumulado debe ignorar posiciones de padding.
- **Errores fieles a la fuente.** El paper aclara que un error copiado de la evidencia o del entrenamiento sale con baja incertidumbre. Ningun detector interno lo atrapa.
- **Precision numerica.** `log(0)` en entropias: usar epsilon 1e-10 como el repo, o calcular con log-softmax.
- **Maquina de build.** 8 GB RAM sin GPU no corre OLMoE-7B ni gpt-oss-20b para tests. Los tests deben usar modelos tiny aleatorios. La validacion de AUROC real necesita GPU externa.

## Objeciones

- **Falta medir el costo aislado del Nivel 1.** Nadie publico la latencia de router entropy y usage sin el resto. La meta < 5% depende de ese dato.
- **Falta un resultado en gpt-oss.** No hay AUROC publicado de senales MoE en gpt-oss-20b o 120b.
- **Falta comparacion contra probe lineal en el mismo setup.** InnerExpert no compara con SAPLMA, SEP ni probes por token. Sin eso, no se sabe si las senales MoE agregan sobre un probe de hidden state.
- **Falta validacion del juez en espanol.** El paper valida GLM-5.1 en ingles con un solo anotador y n=200.
- **Falta resolver la discrepancia de router entropy.** No se sabe si los numeros de la Tabla 3 usan entropia sobre k o sobre N. El codigo sugiere k.
- **Falta confirmar el flag `norm_topk_prob` de OLMoE** y el impacto en la entropia del codigo.
- **Falta la ablacion "sin hidden score ni atencion".** El paper no reporta un IE solo con senales MoE baratas. Ese es justo el producto que Vigia quiere vender.
- **Numeros de HaloScope y CARE no verificados.** Solo lei los abstracts.
- **Parametros activos y vocabulario de gpt-oss** no se confirmaron en esta sesion.

## Fuentes

- Paper InnerExpert (PDF completo leido): https://arxiv.org/pdf/2608.17687
- Repo InnerExpert (MIT): https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
- Metricas del repo: https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/metrics/_metrics.py
- Monitor del repo: https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/monitoring/_llm_monitor.py
- Forwards por modelo: https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/moeuncert/forwards/_model_forwards.py
- Calculo de metricas: https://raw.githubusercontent.com/joaopfonseca/InnerExpert-Hallucination-Detection/HEAD/experiments/0.2-compute-monitor-metrics.py
- gpt-oss en transformers: https://raw.githubusercontent.com/huggingface/transformers/main/src/transformers/models/gpt_oss/modeling_gpt_oss.py
- Config gpt-oss-20b: https://huggingface.co/openai/gpt-oss-20b/blob/main/config.json
- Config gpt-oss-120b: https://huggingface.co/openai/gpt-oss-120b/blob/main/config.json
- vLLM routed experts replay: https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
- Foro vLLM sobre router logits: https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
- INSIDE / EigenScore: https://arxiv.org/html/2402.03744
- Semantic Entropy (Nature 2024, PMC): https://pmc.ncbi.nlm.nih.gov/articles/PMC11186750/
- Semantic Entropy Probes: https://arxiv.org/abs/2406.15927
- SAPLMA: https://arxiv.org/abs/2304.13734
- HaloScope: https://arxiv.org/abs/2409.17504
- Probes de entidades alucinadas: https://arxiv.org/html/2509.03531v2
- Mean shift: https://arxiv.org/abs/2608.28930
- Metricas geometricas: https://arxiv.org/abs/2602.09158
- Pavlitska et al. (MoE en vision): https://arxiv.org/html/2509.04816v1
- CARE: https://arxiv.org/abs/2607.26052
- Bayesian MoE: https://arxiv.org/abs/2509.23830
- Mu-SHROOM ES: https://portal.odesia.uned.es/en/dataset/mu-shroom-2025-es
- Mu-SHROOM paper: https://arxiv.org/abs/2504.11975
