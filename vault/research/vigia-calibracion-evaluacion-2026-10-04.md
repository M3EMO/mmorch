---
title: Vigia - calibracion y evaluacion de detectores de alucinacion por token
mision: Que pipeline de etiquetado, calibracion, seleccion de umbral y reporte de evaluacion debe usar Vigia para que su puntaje por token sea medible, calibrado y auditable.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://arxiv.org/pdf/2608.17687
  - https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
  - https://arxiv.org/html/2509.03531v1
  - https://github.com/obalcells/hallucination_probes
  - https://arxiv.org/html/2504.11975v1
  - https://huggingface.co/datasets/Helsinki-NLP/mu-shroom
  - https://github.com/Helsinki-NLP/mu-shroom
  - https://arxiv.org/html/2401.00396v1
  - https://arxiv.org/html/2502.17125v1
  - https://arxiv.org/pdf/2305.11747
  - https://arxiv.org/abs/2305.14251
  - https://arxiv.org/pdf/2208.02814
  - https://arxiv.org/pdf/2110.01052
  - https://arxiv.org/abs/2402.10978
  - https://arxiv.org/html/2512.15068v1
  - https://arxiv.org/abs/1706.04599
  - https://arxiv.org/pdf/1904.01685
  - https://scikit-learn.org/stable/modules/calibration.html
  - https://arxiv.org/html/2603.21172v1
  - https://arxiv.org/pdf/2406.15927
  - https://www.ncbi.nlm.nih.gov/pmc/articles/PMC10728486/
  - https://metricgate.com/docs/obuchowski-clustered-roc/
  - https://raw.githubusercontent.com/facebookresearch/MLQA/main/mlqa_evaluation_v1.py
  - https://arxiv.org/pdf/2007.15207
  - https://huggingface.co/datasets/PlanTL-GOB-ES/SQAC/tree/main
  - https://aclanthology.org/2025.coling-main.366/
  - https://huggingface.co/datasets/HiTZ/truthfulqa-multi-MT/blob/main/README.md
  - https://github.com/SeldonIO/alibi-detect
  - https://docs.evidentlyai.com/metrics/customize_data_drift
  - https://api-docs.deepseek.com/quick_start/pricing
  - https://huggingface.co/docs/transformers/main/en/model_doc/gpt_oss
  - https://huggingface.co/hf-internal-testing/tiny-random-GptOssForCausalLM/blob/main/tokenizer.json
  - https://huggingface.co/hf-internal-testing/tiny-random-OlmoeForCausalLM/blob/main/tokenizer.json
  - https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM/blob/main/generation_config.json
  - https://arxiv.org/html/2505.04847v1
created: 2026-10-04
---

## Tronco

Vigia debe entrenar un detector lineal sobre generaciones propias del modelo, etiquetadas una vez por un juez LLM con razonamiento (mas un set computable con respuesta gold para CI), calibrarlo con Platt sobre un split agrupado por pregunta, fijar el umbral con conformal risk control sobre la tasa de falsos negativos, y reportar AUROC/AUPRC/ECE/Brier/recall@FPR con intervalos bootstrap por cluster de respuesta.

## Hallazgos

### 1. La referencia directa: InnerExpert (Fonseca, Rodrigues, Romano 2026)

Lei el PDF completo (arXiv 2608.17687, v1 del 18 ago 2026, AAAI 2027). El codigo esta en GitHub (ver Fuentes). Estos son los datos que importan para calibracion y evaluacion.

**Etiquetado.**

- El juez es GLM-5.1 via DeepInfra.
- El juez recibe pregunta, evidencia y respuesta. Devuelve JSON con `label` (0/1) y `hallucinated_spans` (substrings exactos de la respuesta).
- El prompt del juez es corto. Exige substrings exactos y lista vacia si `label = 0`.
- Un token es positivo si su span de caracteres se solapa con algun span alucinado.
- Cada pregunta genera dos respuestas: una sin evidencia (base) y una con evidencia (simula RAG). Esto crea diversidad de etiquetas.
- La generacion usa greedy, maximo 65 tokens nuevos.
- Las features se calculan solo sobre tokens generados, nunca sobre el prompt.

**Validacion del juez contra humano (Apendice F.2).**

| Humano \ Juez | Grounded | Hallucinated | Total |
|---|---|---|---|
| Grounded | 93 | 20 | 113 |
| Hallucinated | 7 | 80 | 87 |
| Total | 100 | 100 | 200 |

- La exactitud del juez es 86.5% (27 errores sobre 200).
- El juez marca falsos positivos con mas frecuencia: 20 de 113 grounded (17.7%) contra 7 de 87 alucinados (8.0%).
- La heuristica debil (BLEU/ROUGE/BERTScore) logra solo 59.5%.
- La validacion usa un solo anotador humano y etiquetas a nivel respuesta, no a nivel token.

**Splits y umbral.**

- El split train/validacion esta estratificado por tasa de alucinacion y agrupado por id de pregunta.
- La validacion es 10% del train.
- El umbral se elige maximizando F1 sobre todos los datos etiquetados.
- La evaluacion OOD es temporal: RealtimeQA ene-jun 2026 (365 preguntas). Hay cuatro datasets cruzados de 200 preguntas cada uno.

**Resultados por detector (AUROC token, promedio de 5 datasets).**

| Detector | OLMoE token AUROC | Gemma-4-26B token AUROC | OLMoE token F1 | Gemma token F1 |
|---|---|---|---|---|
| Regresion logistica | 0.762 | 0.725 | 0.754 | 0.693 |
| MLP | 0.754 | 0.730 | 0.774 | 0.675 |
| XGBoost | 0.736 | 0.753 | 0.613 | 0.276 |
| Random Forest | 0.594 | 0.710 | 0.660 | 0.676 |
| Transformer | 0.619 | 0.694 | 0.671 | 0.636 |
| Entropia de logits (baseline) | 0.639 | 0.541 | 0.647 | 0.382 |

- XGBoost gana en AUROC en Gemma pero su F1 colapsa a 0.276. El paper lo atribuye a que el umbral F1-optimo no generaliza entre datasets.
- Esto es la evidencia mas fuerte para Vigia: el ranking es bueno, pero el umbral sin calibrar no transfiere.
- A nivel respuesta, el mejor promedio es 0.912 (XGB, Gemma) y 0.882 (XGB, OLMoE).

**Senales individuales.**

- Ninguna senal MoE es fuerte en ambos modelos.
- La entropia del router es anti-correlacionada en Gemma (0.327 AUROC respuesta, 0.446 token).
- Inv. Herfindahl da 0.675 token en OLMoE. Gini da 0.626 token en Gemma.
- La combinacion siempre supera a la mejor senal sola.

**Costo de inferencia (Tabla 14, segundos por 100 tokens).**

| Metodo | OLMoE | Gemma |
|---|---|---|
| Vanilla | 1.146 | 3.825 |
| Entropia de logits | 1.152 | 3.830 |
| Senales MoE completas | 2.829 | 7.677 |
| InnerExpert (LR) | 3.493 | 9.376 |
| Semantic Uncertainty (K=5) | 6.722 | 21.860 |

- InnerExpert completo cuesta ~3x vanilla. La meta del plan de Vigia es < 5%.
- Las senales estandar baratas agregan < 10%.
- La memoria GPU sube ~3% (12.66 a 13.08 GB en OLMoE).
- El proyecto ocupo ~1.7 TB en disco, mayormente lotes crudos.
- Las features son acumulativas sobre el prefijo. El hidden score usa SVD por paso. Eso explica gran parte del costo.

**Distribucion de clases.**

- OLMoE: 83.0% de respuestas alucinadas (99.7% sin evidencia).
- Gemma: 49.4% alucinadas.
- El desbalance infla el F1. AUPRC y la tasa base deben reportarse siempre.

**Limitaciones declaradas.** Solo ingles. Validacion humana chica. No evalua gpt-oss, Mixtral, Qwen-MoE ni DeepSeek.

### 2. La referencia de probes por token: Obeso et al. 2025 (arXiv 2509.03531)

- El juez es Claude 4 Sonnet con busqueda web. Extrae entidades y las verifica.
- Las etiquetas son "Supported", "Not Supported", "Insufficient Information". Las dos ultimas cuentan como alucinadas.
- Todos los tokens de una entidad heredan la etiqueta de la entidad.
- Acuerdo juez-humano: 84% (n=50).
- Sobre errores inyectados: recall 80.6% con FPR 15.8% (100 muestras).
- La perdida combina BCE por token (peso 10 en tokens de entidad, 1 en fondo) y BCE sobre el maximo del span. El peso del termino span se recoce de 0 a 1.
- La metrica principal es AUC y recall a 10% FPR (R@0.1).
- La agregacion por entidad es span-max.
- Resultado en Llama-3.3-70B con LoRA: 0.90 AUC en LongFact, 0.91 en HealthBench, 0.98 en TriviaQA.
- Transferencia entre modelos: los AUC fuera de la diagonal quedan a 0.02-0.04 de la diagonal. Un probe entrenado con datos de Llama-3.1-8B rinde casi igual en Mistral-Small-24B (diferencia ≤ 0.02).
- El paper no trata la calibracion de forma explicita.
- El paper no evalua idiomas distintos del ingles.

### 3. Etiquetado con juez LLM a nivel token/span

**Protocolo recomendado (sintesis de InnerExpert, Obeso, RAGTruth, FaithJudge).**

1. El juez recibe pregunta, evidencia de referencia y respuesta.
2. El juez devuelve JSON con etiqueta global y spans como substrings exactos.
3. Un validador determinista verifica que cada span sea substring exacto. Si no lo es, el item vuelve al juez o se descarta.
4. Un mapeador convierte spans de caracteres a etiquetas por token usando offsets del tokenizer del modelo anfitrion.
5. El juez opera con razonamiento prendido. La memoria local de mmorch midio que el mismo modelo sin thinking rechaza ~40% del trabajo correcto (acc. balanceada 0.77 contra 0.997). El sesgo de InnerExpert (17.7% de falsos alucinados) apunta en la misma direccion.
6. FaithJudge mejora el acuerdo con humanos dando al juez ejemplos anotados del mismo articulo (few-shot con anotaciones humanas).

**Fuentes de etiqueta comparadas.**

| Fuente | Granularidad | Costo | Ruido conocido | Uso en Vigia |
|---|---|---|---|---|
| Humano experto (RAGTruth) | palabra | alto | RAGTruth no publica kappa | gold chico de auditoria |
| Juez con evidencia (InnerExpert) | span substring | muy bajo | 86.5% acc. vs humano, sesgo a falso positivo | calibracion principal |
| Juez con web search (Obeso) | entidad | medio | 84% acuerdo, FPR 15.8% en inyectados | opcional sin evidencia |
| Respuesta gold exacta (EM) | respuesta corta | cero | falsos negativos por parafrasis | CI determinista |
| Aritmetica / fechas sinteticas | token del numero | cero | casi nulo | CI determinista |
| Heuristica BLEU/ROUGE | respuesta | cero | 59.5% acc. (InnerExpert) | no usar |

**Acuerdo inter-evaluador humano (Mu-SHROOM, SemEval-2025 Task 3).**

- Cada salida tiene al menos tres anotadores. Las etiquetas son por caracter, con version binaria y probabilidad blanda.
- IoU entre anotadores: italiano 0.87, hindi 0.80, chino 0.58, ingles 0.49, **espanol 0.51** (validacion).
- Esto fija un techo: los humanos coinciden en la mitad del span en espanol. Un detector no puede superar con confianza ese techo medido contra humanos.
- Las metricas oficiales son IoU sobre etiquetas duras (>50% de anotadores) y Spearman contra las etiquetas blandas.

**Costo del juez.**

Precios leidos hoy en la pagina oficial de DeepSeek (rango off-peak / peak, USD por 1M tokens):

| Modelo | Input cache hit | Input cache miss | Output |
|---|---|---|---|
| deepseek-flash (V4.1-Flash) | 0.003-0.006 | 0.15-0.3 | 0.6-1.2 |
| deepseek-v4-pro | 0.022-0.044 | 0.66-1.32 | 1.98-3.96 |

- Estimacion propia (no medida): 5 000 respuestas x ~700 tokens de entrada = 3.5M tokens. Eso cuesta ~USD 1 de entrada en flash peak.
- Con razonamiento, la salida puede llegar a ~1 000 tokens por item. Eso suma ~5M tokens de salida, ~USD 6 en flash peak.
- El costo del juez es despreciable frente al costo de GPU para generar con el modelo anfitrion.
- FActScore estima que 6 500 generaciones evaluadas por humanos costarian USD 26K. Su estimador automatico tiene < 2% de error a nivel agregado.

### 4. Datasets de referencia

| Dataset | Idioma | Tamano | Nivel de etiqueta | Generado por | Licencia | Utilidad para Vigia |
|---|---|---|---|---|---|---|
| RAGTruth | en | 2 973 instancias, 17 838 respuestas | palabra, 4 tipos | GPT-3.5/4, Mistral-7B, Llama-2 7/13/70B | [no verificado] | off-policy; benchmark de spans RAG |
| HaluEval | en | 35 000 (5 000 humanas) | respuesta | ChatGPT | [no verificado] | off-policy; solo nivel respuesta |
| FActScore | en | biografias | hecho atomico | varios | [no verificado] | metodologia de long-form |
| Mu-SHROOM | 14 idiomas, incluye es | es: 50 val + 150 test; total 499 val, 1.9k test, 3.35k train sin etiqueta | caracter, blanda y dura | 38 LLMs abiertos | HF dice CC-BY-4.0; paper dice CC BY-NC-SA 4.0 | unico benchmark de spans en espanol |
| RealtimeQA | en | stream | respuesta + evidencia | (pipeline propio) | [no verificado] | OOD temporal |
| SQuAD, NQ-Open, TruthfulQA, FreshQA | en | 200 c/u en InnerExpert | gold | - | varias | gold computable |
| MKQA | 26 idiomas, incluye es | 10k pares por idioma | gold ligado a Wikidata | traduccion humana de NQ | [no verificado] | QA factual es con gold computable |
| SQAC | es | 18 817 preguntas, 6 247 contextos | span extractivo | anotadores nativos | CC-BY-SA-4.0 | RAG en espanol con gold |
| MLQA / XQuAD | incluye es | [no verificado] | span extractivo | humano | [no verificado] | RAG es; MLQA trae normalizador es |
| VeritasQA | es, ca, gl, en | 353 preguntas | respuesta | revision manual de TruthfulQA | abierta [no verificado cual] | conceptos erroneos en espanol |
| TruthfulQA-multi-MT | en, es, ca, gl, eu | 817 preguntas | respuesta | traduccion automatica (Claude 3.5 Sonnet) | Apache 2.0 | conceptos erroneos, traduccion MT |

**Punto clave: on-policy contra off-policy.**

- Las senales internas (router, expertos) solo existen cuando el modelo anfitrion procesa el texto.
- RAGTruth y HaluEval tienen texto de otros modelos. Para usarlos hay que pasar ese texto por el anfitrion con teacher forcing. Eso es off-policy.
- InnerExpert y Obeso entrenan sobre generaciones propias.
- Obeso reporta transferencia entre modelos a 0.02-0.04 AUC. Eso sugiere que el texto off-policy sirve. No hay dato equivalente para senales MoE [no verificado].
- Mu-SHROOM incluye `model_output_tokens` y `model_output_logits` de modelos distintos. Sirve para evaluar spans en espanol, no para entrenar senales del router.

### 5. Etiquetas computables sin LLM (para CI determinista)

**Normalizacion de respuesta en espanol.** El script oficial de MLQA hace: minusculas, quita puntuacion Unicode, quita articulos y normaliza espacios. Para `es` el regex es `\b(un|una|unos|unas|el|la|los|las)\b`. Vigia puede reusar esta logica (codigo propio, misma regla).

**Reglas de etiqueta computable.**

| Tarea | Gold | Etiqueta respuesta | Etiqueta token |
|---|---|---|---|
| QA factual corto (MKQA, SQAC) | lista de alias | 1 si ningun alias coincide por EM normalizado | tokens del span de respuesta = etiqueta; resto enmascarado |
| Aritmetica sintetica en espanol | resultado exacto | 1 si el numero difiere | tokens del numero = etiqueta |
| Fechas / conteos sinteticos | valor exacto | 1 si difiere | tokens del valor |
| Extraccion desde contexto (SQAC) | span en contexto | 1 si no es substring del contexto | tokens fuera del contexto = 1 |

- LettuceDetect enmascara contexto y pregunta con `-100` y etiqueta solo tokens de respuesta. Vigia debe usar la misma convencion de mascara.
- El EM produce falsos positivos por parafrasis ("Buenos Aires" contra "la Ciudad Autonoma de Buenos Aires"). El token F1 de SQuAD/MLQA mitiga eso. Hay que fijar un umbral de F1 o usar listas de alias.

**CI en CPU con 8 GB de RAM.**

- Hugging Face publica modelos aleatorios diminutos: `hf-internal-testing/tiny-random-GptOssForCausalLM`, `tiny-random-OlmoeForCausalLM` y `tiny-random-MixtralForCausalLM`.
- Esos modelos sirven para tests de forma y de contrato del extractor (hooks, dimensiones, router logits). No sirven para medir AUROC real porque sus pesos son aleatorios.
- La documentacion de transformers expone `output_router_logits` en `GptOssConfig`. Devuelve una tupla por capa con forma `(batch, seq, num_experts)`.
- Los tests de calibracion y metricas deben usar features sinteticas con senal plantada y semilla fija. El resultado esperado se calcula de forma analitica o se congela como fixture.

### 6. Detectores livianos y su latencia

- InnerExpert usa scikit-learn 1.8. Las grillas fueron: LR `C ∈ {0.01, 0.1, 1, 10}`; XGB `n_estimators ∈ {300, 500, 1000}`, `max_depth ∈ {3, 6, 10}`, `lr ∈ {0.001, 0.01, 0.1}`; MLP `hidden ∈ {128, 256, 512, 1024}`, `alpha ∈ {1e-4 … 1e-1}`.
- La dimension por token es `5L + LH + LN`. Para OLMoE es 1 360. Para Gemma-4-26B es 4 470.
- Para gpt-oss-20b la dimension depende de L, H y N de su config [no verificado en esta sesion]. Con la formula, una config grande lleva a miles de features. La LR sigue siendo un producto punto.
- En la Tabla 14, pasar de "senales MoE" a "IE (LR)" agrega ~0.66 s por 100 tokens en OLMoE. Eso es costo de la implementacion de investigacion, no del producto punto [inferencia propia].
- Semantic Entropy Probes (Kossen et al. 2024) usan regresion logistica sobre hidden states. Reducen el costo de semantic entropy (5-10x) a casi cero.

**Comparacion para Vigia.**

| Detector | Ventaja | Riesgo | Recomendacion |
|---|---|---|---|
| LR (L2) | estable entre modelos, rapido, interpretable, calibrable con Platt | no capta interacciones | default |
| MLP chico | mejor F1 medio en InnerExpert | requiere early stopping y semilla | opcion avanzada |
| XGBoost | mejor AUROC en Gemma | umbral inestable (F1 0.276) | solo con calibracion y CRC |
| Random Forest | robusto a escala | AUROC bajo en OLMoE (0.594) | no |
| Transformer encoder | contexto secuencial | inestable y caro | no en MVP |

### 7. Calibracion

| Metodo | Parametros | Cuando usar | Nota |
|---|---|---|---|
| Platt (sigmoide) | 2 | < ~1 000 muestras o error simetrico | conserva el ranking y el AUROC |
| Isotonica | no parametrico | > ~1 000 muestras | puede introducir empates y cambiar el AUROC |
| Temperature scaling | 1 | multiclase con logits | en binario equivale a Platt sin sesgo |
| Ensemble CV (`CalibratedClassifierCV`) | k pares | entrenamiento | `ensemble=False` da un solo modelo mas rapido |
| `FrozenEstimator` | - | recalibrar un modelo ya entrenado | exige datos disjuntos del entrenamiento |

- Las citas de scikit-learn: isotonica rinde igual o mejor "when there is enough data (greater than ~ 1000 samples)". Para conservar el AUROC, usar sigmoide.
- Guo et al. (2017) muestran que las redes modernas estan mal calibradas y que temperature scaling es sorprendentemente efectivo.
- El conteo que importa es de respuestas, no de tokens. Los tokens de una misma respuesta estan correlacionados.
- La calibracion del puntaje de respuesta es aparte. El maximo de probabilidades calibradas por token no es una probabilidad calibrada de respuesta [inferencia propia].
- Brier bajo no implica buena calibracion. Puede venir de mayor poder discriminativo (scikit-learn).

### 8. Metricas

| Metrica | Nivel | Que mide | Detalle de implementacion |
|---|---|---|---|
| AUROC | token, span, respuesta | ranking | DeLong o bootstrap por cluster |
| AUPRC | token, respuesta | ranking bajo desbalance | reportar junto a la tasa base |
| ECE | token, respuesta | calibracion | 15 bins de igual masa; reportar tambien ACE |
| Brier y log loss | token, respuesta | reglas propias | descomponer en calibracion y resolucion |
| Curva de confiabilidad | token, respuesta | calibracion visual | bins de igual masa con conteo por bin |
| Precision@recall | token, respuesta | operacion | recall objetivo 0.8 y 0.9 |
| Recall@FPR (R@0.1) | token, respuesta | operacion | metrica de Obeso |
| F1 al umbral elegido | token, span, respuesta | operacion | nunca elegir el umbral en el test |
| IoU de span | span | localizacion | metrica de Mu-SHROOM |
| Spearman vs etiquetas blandas | caracter | localizacion graduada | metrica de Mu-SHROOM |
| F1 por caracter | span | localizacion | metrica de LettuceDetect y RAGTruth |
| AURC / E-AURC | respuesta | abstencion selectiva | menor es mejor |
| TCE | respuesta | error frente a riesgo objetivo | rango operativo [0.05, 0.30] |
| Sobrecosto de latencia | sistema | costo | p50 y p95, en % sobre vanilla |

- Nixon et al. (2019) muestran que el ECE con bins uniformes es inestable. Los esquemas adaptativos (ACE) dan rankings mas estables.
- El paper "Entropy Alone is Insufficient" muestra un regimen "confidently wrong". Un AUROC alto no garantiza abstencion segura a riesgo estricto. Ese paper recomienda E-AURC y TCE.
- Referencias de span en RAGTruth: GPT-4-turbo con prompt logra 32.7% F1 por span; Llama-2-13B ajustado logra 54.8%. LettuceDetect large logra 58.93% F1 por span y 79.22% por respuesta.

### 9. Seleccion de umbral con garantia

**Problema.** InnerExpert elige el umbral por F1 sobre datos etiquetados. XGBoost muestra que ese umbral no transfiere.

**Conformal risk control (Angelopoulos et al. 2022).**

- La perdida debe ser no creciente en λ y acotada por B.
- Con `R̂_n(λ)` como riesgo empirico en n puntos de calibracion: `λ̂ = inf{ λ : (n/(n+1)) R̂_n(λ) + B/(n+1) ≤ α }`.
- Garantia: `E[L_{n+1}(λ̂)] ≤ α`. Con datos i.i.d. continuos, la cota inferior es `α − 2B/(n+1)`.
- Supuesto: intercambiabilidad. Si la perdida no es monotona, el metodo no controla el riesgo.
- El paper muestra control de FNR en multilabel y segmentacion.

**Mapeo a Vigia.**

- El conjunto marcado es `C_λ = { t : s(t) ≥ 1 − λ }`.
- La perdida por respuesta es la fraccion de tokens alucinados no marcados: `1 − |Y ∩ C_λ| / |Y|`. Es monotona y acotada por B = 1.
- Las respuestas sin tokens alucinados tienen perdida 0. Hay que definir si cuentan en n.
- Con α = 0.1, Vigia marca en promedio ≥ 90% de los tokens alucinados.

**Learn then Test (Angelopoulos, Bates, Candes, Jordan, Lei).**

- Reformula el control de riesgo como test multiple de hipotesis.
- Da garantias en alta probabilidad (1 − δ), no solo en esperanza.
- Permite controlar varios riesgos a la vez (por ejemplo FNR y tasa de marcado).

**Advertencia empirica (The Semantic Illusion, arXiv 2512.15068).**

- Con SCP, detectores por embeddings logran 95% de cobertura con 0% FPR en alucinaciones sinteticas.
- En alucinaciones reales (HaluEval) los mismos metodos llegan a 100% FPR a la cobertura objetivo.
- Conclusion: la garantia conformal se cumple, pero el costo en falsos positivos puede ser inaceptable. Vigia debe reportar la tasa de marcado junto a la garantia.

**Conformal factuality (Mohri y Hashimoto 2024).** Usa back-off: quita afirmaciones hasta alcanzar la garantia. Logra 80-90% de correccion conservando la mayor parte de la salida. Es el analogo de la accion "abstencion parcial" de Vigia.

### 10. Agregacion token → span → respuesta

| Operador | Uso | Fuente |
|---|---|---|
| max | respuesta y entidad | InnerExpert (respuesta), Obeso (span-max) |
| media | respuesta | InnerExpert lo evalua como alternativa |
| top-k media | respuesta larga | [propuesta propia, no verificada] |
| noisy-or `1 − ∏(1 − p_t)` | respuesta | [propuesta propia; sobreestima con tokens correlacionados] |

**Construccion de spans (propuesta).**

1. Marcar tokens con `s(t) ≥ τ`.
2. Unir tokens contiguos y cerrar huecos de un token.
3. Expandir a limites de palabra con offsets de caracteres.
4. Puntaje del span = maximo del span.
5. Descartar spans de solo puntuacion o espacios.

- Las features de InnerExpert son acumulativas sobre el prefijo. El puntaje de un token depende de la longitud ya generada. Hay que estratificar las metricas por posicion.

### 11. Calibracion por cliente y drift

**Calibracion por cliente.**

- El detector base se entrena una vez por modelo anfitrion.
- Cada cliente aporta un set propio etiquetado por juez, en su dominio y en espanol.
- Con ese set se reajusta solo el calibrador (Platt) y el umbral CRC. El detector queda congelado (`FrozenEstimator`).
- Si el set del cliente es chico, Platt es la opcion segura (2 parametros).

**Drift.**

- Evidently usa por defecto PSI con umbral 0.1 y Wasserstein con umbral 0.1 (columnas numericas con > 1 000 objetos). Marca drift de dataset si cambia 50% de las columnas.
- alibi-detect ofrece KS, CVM, MMD, LSDD, Chi-cuadrado y detectores por clasificador. Los detectores online son Online MMD, Online LSDD y context-aware MMD. Soporta backends TensorFlow y PyTorch. La licencia no figura en la pagina leida [no verificado].
- Vigia debe vigilar tres cosas: la distribucion de features (sobre todo entropia del router), la distribucion del puntaje y la tasa de marcado.
- El drift de etiquetas requiere auditorias periodicas: re-etiquetar con el juez una muestra de produccion y recalcular AUROC y FNR.
- InnerExpert propone el pipeline no supervisado como monitoreo online de la tasa de alucinacion (Apendice G).

### 12. Intervalos de confianza

- Los tokens de una respuesta no son independientes. El bootstrap ingenuo por token subestima la varianza.
- El bootstrap por cluster remuestrea respuestas (o preguntas) con reemplazo y conserva los tokens dentro de cada cluster.
- Obuchowski extiende DeLong a datos agrupados. Es no parametrico y no requiere estructura de correlacion.
- Para comparar dos detectores, usar bootstrap pareado: el mismo remuestreo para ambos.
- Recomendacion: 2 000 replicas, intervalo percentil al 95%, cluster = id de pregunta [numero de replicas es convencion, no fuente].

### Pipeline de calibracion recomendado (paso a paso)

1. **Fijar el anfitrion.** Modelo, cuantizacion, plantilla de chat y decodificacion son parte de la version del detector.
2. **Armar el banco de prompts.** Mezclar QA factual en espanol con gold (MKQA, SQAC), RAG con evidencia (SQAC, contexto del cliente) y preguntas sin evidencia. Generar dos respuestas por pregunta: con evidencia y sin evidencia.
3. **Generar on-policy con instrumentacion.** Guardar router logits por capa y token, features compactas y offsets de caracteres. No guardar tensores crudos por experto (InnerExpert llego a 1.7 TB).
4. **Etiquetar.** Juez con razonamiento prendido devuelve JSON con label y spans. Validar substrings. Mapear spans a tokens. Para items con gold, calcular tambien la etiqueta computable.
5. **Auditar el juez.** Comparar juez contra gold computable en el subconjunto que lo tiene. Muestrear 100-200 items para revision humana en espanol. Registrar matriz de confusion y kappa.
6. **Particionar.** Train / calibracion / test agrupados por id de pregunta. Agregar un test OOD temporal o de dominio.
7. **Entrenar el detector.** LR con estandarizacion z-score. Seleccionar C por AUROC en validacion agrupada.
8. **Calibrar.** Platt sobre el split de calibracion (isotonica solo con > 1 000 respuestas). Calibrar por separado el puntaje de respuesta.
9. **Fijar el umbral.** CRC sobre FNR por token con α elegido por el cliente. Reportar tambien la tasa de marcado y la precision resultantes.
10. **Evaluar en test.** Reporte canonico (abajo) con IC bootstrap por cluster.
11. **Medir latencia.** p50 y p95 de sobrecosto frente a vanilla, con el mismo hardware y la misma longitud.
12. **Versionar y desplegar.** Guardar detector, calibrador, umbral, hash del dataset y hash del prompt del juez.
13. **Monitorear.** PSI de features y puntaje, tasa de marcado, auditoria mensual con juez.
14. **Recalibrar por cliente.** Repetir pasos 4-10 con datos del cliente y el detector congelado.

### Reporte de evaluacion canonico

**Encabezado.** Modelo anfitrion y version. Hash del detector. Dataset y tamano (respuestas y tokens). Tasa base de alucinacion. Juez y version de prompt. Fecha.

**Tabla 1: discriminacion.** AUROC y AUPRC por nivel (token, span, respuesta) y por dataset, con IC 95% por cluster.

**Tabla 2: calibracion.** ECE (15 bins de igual masa), ACE, Brier, log loss, antes y despues de calibrar.

**Tabla 3: operacion.** Umbral elegido, α objetivo, FNR observado, precision, tasa de marcado, F1, recall@FPR 0.1.

**Tabla 4: localizacion.** IoU de span, F1 por caracter, Spearman contra etiquetas blandas (si hay Mu-SHROOM).

**Tabla 5: costo.** Latencia p50/p95 vanilla y con Vigia, sobrecosto %, memoria pico.

**Tabla 6: calidad de etiquetas.** Matriz juez contra humano y juez contra gold computable.

**Graficos.**

1. Curva ROC por nivel, con banda bootstrap.
2. Curva precision-recall con linea de tasa base.
3. Diagrama de confiabilidad con histograma de conteos por bin.
4. Curva de riesgo-cobertura (AURC) para la accion de abstencion.
5. FNR observado contra α objetivo (validacion de CRC).
6. AUROC por posicion de token y por longitud de respuesta.
7. Histograma del puntaje separado por clase.
8. Serie temporal de PSI y tasa de marcado (reporte de drift).

## Implicancias para Vigia

1. **Requisito R1:** el modulo `labeling` debe aceptar un juez configurable con razonamiento prendido y devolver JSON validado contra un schema (`label`, `hallucinated_spans`).
2. **Requisito R2:** un validador determinista rechaza spans que no sean substring exacto y resuelve ocurrencias repetidas por posicion.
3. **Requisito R3:** el mapeo span → token usa offsets de caracteres del tokenizer del anfitrion y marca positivo por solapamiento.
4. **Requisito R4:** el modulo `computable_labels` implementa EM y token-F1 con normalizacion en espanol (articulos el/la/los/las/un/una/unos/unas) y verificacion aritmetica.
5. **Requisito R5:** los tests de CI corren en CPU con features sinteticas de semilla fija y con `tiny-random-GptOssForCausalLM`, `tiny-random-OlmoeForCausalLM` y `tiny-random-MixtralForCausalLM` para contratos de forma.
6. **Requisito R6:** el detector default es regresion logistica con estandarizacion. MLP queda como opcion. XGBoost solo se habilita con calibracion obligatoria.
7. **Requisito R7:** todo split se agrupa por id de pregunta. El codigo debe impedir que dos respuestas de la misma pregunta caigan en splits distintos.
8. **Requisito R8:** el calibrador default es Platt. La isotonica se habilita solo con > 1 000 respuestas de calibracion.
9. **Requisito R9:** el umbral se fija con conformal risk control sobre FNR por token. El usuario elige α en el panel.
10. **Requisito R10:** el panel muestra junto a α la tasa de marcado y la precision esperadas. Una garantia de FNR sin costo visible engana.
11. **Requisito R11:** el reporte de evaluacion genera las 6 tablas y los 8 graficos listados, con IC bootstrap por cluster (2 000 replicas).
12. **Requisito R12:** el reporte siempre muestra la tasa base junto a AUPRC y F1.
13. **Requisito R13:** el puntaje de respuesta tiene su propio calibrador. No se reutiliza el maximo de probabilidades por token como probabilidad.
14. **Requisito R14:** un artefacto de detector incluye modelo anfitrion, cuantizacion, plantilla de chat, decodificacion, version del juez, hash del dataset, calibrador y umbral. Un cambio en cualquiera invalida la calibracion.
15. **Requisito R15:** el monitor de drift calcula PSI (umbral 0.1) sobre puntaje, tasa de marcado y features clave, y dispara una auditoria con juez.
16. **Requisito R16:** la recalibracion por cliente congela el detector y reajusta solo calibrador y umbral.
17. **Decision D1:** el MVP no usa las features costosas de InnerExpert (SVD acumulado del hidden score, per-expert hidden score, attention score). Empieza por entropia del router, Gini e inv. Herfindahl de uso, entropia de logits y similitud entre expertos activos. Medir AUROC y latencia antes de agregar mas.
18. **Decision D2:** la meta < 5% de latencia se mide contra vanilla con la misma longitud y el mismo hardware. InnerExpert completo cuesta ~3x. La meta exige features baratas o computo asincrono.
19. **Decision D3:** el dataset de calibracion en espanol combina MKQA-es y SQAC (gold computable), VeritasQA y TruthfulQA-multi (conceptos erroneos) y datos del cliente (juez). Mu-SHROOM-es sirve solo como benchmark de spans.
20. **Decision D4:** el juez default debe poder ser local o autoalojado. Los clientes de salud, finanzas y gobierno pueden prohibir enviar datos a una API externa.
21. **Decision D5:** el almacenamiento guarda features compactas por token, no tensores crudos por experto.
22. **Decision D6:** los tokens del canal de razonamiento de gpt-oss se excluyen del puntaje visible, o se reportan aparte [hipotesis; formato de canales no verificado en esta sesion].

## Edge cases y riesgos

- **Juez sesgado a falso positivo.** InnerExpert mide 17.7% de grounded marcados como alucinados. El detector aprende ese sesgo. Mitigacion: razonamiento prendido y auditoria contra gold.
- **Spans no exactos.** El juez puede devolver texto parafraseado o con espacios distintos. El validador debe rechazarlo, no adivinar.
- **Substring repetido.** "2020" puede aparecer dos veces en la respuesta. El juez debe indicar la ocurrencia o el mapeo marca ambas.
- **Respuestas de abstencion.** "No tengo esa informacion" puede etiquetarse como grounded o como alucinada segun el juez. InnerExpert muestra ambas decisiones en sus ejemplos. El prompt del juez debe fijar la regla.
- **Premisas falsas.** FreshQA incluye preguntas con premisa falsa. Una respuesta que acepta la premisa es alucinada aunque no invente entidades.
- **Desbalance extremo.** OLMoE sin evidencia alucina 99.7%. Con tasas asi, el F1 y la exactitud no informan.
- **Desajuste de decodificacion.** InnerExpert calibra con greedy. Si produccion usa muestreo con temperatura, las senales cambian [no verificado cuanto].
- **Cuantizacion distinta.** Calibrar en bf16 y servir en 4 bits puede cambiar el ruteo [no verificado].
- **Features acumulativas.** El puntaje depende de la posicion. Respuestas largas en produccion pueden salir de la distribucion de calibracion (InnerExpert usa 65 tokens max).
- **Intercambiabilidad rota.** CRC asume intercambiabilidad. El drift de dominio la rompe y la garantia deja de valer.
- **Calibracion con pocos datos por cliente.** Con decenas de respuestas, el termino B/(n+1) de CRC es grande y el umbral queda muy conservador.
- **Empates de isotonica.** La isotonica puede cambiar el AUROC por empates.
- **Techo humano en espanol.** IoU 0.51 entre anotadores. Las metricas de span contra un solo anotador son ruidosas.
- **Licencia de Mu-SHROOM.** HF dice CC-BY-4.0. El paper dice CC BY-NC-SA 4.0. Un uso comercial requiere confirmar.
- **Licencia NC en otros datasets.** Varias licencias quedan sin verificar. No embeber datasets en el producto sin revisar.
- **Fuga temporal.** Si el juez o el anfitrion vieron las respuestas en su entrenamiento, la etiqueta pierde valor. RealtimeQA mitiga eso con preguntas posteriores al corte.
- **Privacidad.** Enviar datos de clientes de salud o finanzas a un juez externo puede violar normas locales de proteccion de datos [detalle legal no verificado].
- **Memoria en CPU.** Ningun MoE real objetivo entra en 8 GB de RAM. La calibracion real exige GPU del cliente o nube. CI solo valida contratos.
- **Texto off-policy.** Entrenar con RAGTruth por teacher forcing puede degradar senales del router [no verificado para MoE].

## Objeciones

- **No hay evidencia en espanol.** InnerExpert y Obeso son solo ingles. No sabemos si el AUROC por token se sostiene en espanol.
- **No hay evidencia en gpt-oss.** InnerExpert usa OLMoE y Gemma-4. La entropia del router es anti-correlacionada en Gemma. El signo de cada senal en gpt-oss es desconocido.
- **La meta de 5% no tiene respaldo.** El unico costo medido es ~3x vanilla para el set completo. No hay medicion del subconjunto barato.
- **La validacion del juez es debil.** Un anotador, 200 items, nivel respuesta. No existe un acuerdo juez-humano medido a nivel token.
- **La garantia CRC no se valido en este problema.** Ningun paper leido aplica CRC a FNR por token sobre senales MoE.
- **El costo del juez es estimado.** Los numeros de tokens por item son supuestos propios. Los precios de DeepSeek muestran rangos off-peak/peak, leidos hoy.
- **Configuracion de gpt-oss.** No verifique L, H y N de gpt-oss-20b en esta sesion. La dimension de features queda abierta.
- **Dato faltante clave:** un experimento chico en un MoE real que mida AUROC por token y latencia del subconjunto barato, en espanol, con etiquetas computables.

## Fuentes

- InnerExpert, Fonseca, Rodrigues, Romano (2026). https://arxiv.org/pdf/2608.17687
- Codigo InnerExpert. https://github.com/joaopfonseca/InnerExpert-Hallucination-Detection
- Obeso et al. (2025), Real-Time Detection of Hallucinated Entities. https://arxiv.org/html/2509.03531v1
- Codigo hallucination_probes. https://github.com/obalcells/hallucination_probes
- Mu-SHROOM, SemEval-2025 Task 3. https://arxiv.org/html/2504.11975v1
- Dataset Mu-SHROOM. https://huggingface.co/datasets/Helsinki-NLP/mu-shroom
- Repo Mu-SHROOM. https://github.com/Helsinki-NLP/mu-shroom
- RAGTruth (ACL 2024). https://arxiv.org/html/2401.00396v1
- LettuceDetect. https://arxiv.org/html/2502.17125v1
- HaluEval. https://arxiv.org/pdf/2305.11747
- FActScore. https://arxiv.org/abs/2305.14251
- Conformal Risk Control, Angelopoulos et al. https://arxiv.org/pdf/2208.02814
- Learn then Test. https://arxiv.org/pdf/2110.01052
- Conformal Factuality, Mohri y Hashimoto. https://arxiv.org/abs/2402.10978
- The Semantic Illusion. https://arxiv.org/html/2512.15068v1
- On Calibration of Modern Neural Networks, Guo et al. https://arxiv.org/abs/1706.04599
- Measuring Calibration in Deep Learning, Nixon et al. https://arxiv.org/pdf/1904.01685
- scikit-learn, Probability calibration. https://scikit-learn.org/stable/modules/calibration.html
- Entropy Alone is Insufficient for Safe Selective Prediction in LLMs. https://arxiv.org/html/2603.21172v1
- Semantic Entropy Probes. https://arxiv.org/pdf/2406.15927
- Bootstrap de AUC con datos correlacionados. https://www.ncbi.nlm.nih.gov/pmc/articles/PMC10728486/
- Obuchowski clustered ROC. https://metricgate.com/docs/obuchowski-clustered-roc/
- Script de evaluacion MLQA. https://raw.githubusercontent.com/facebookresearch/MLQA/main/mlqa_evaluation_v1.py
- MKQA. https://arxiv.org/pdf/2007.15207
- SQAC. https://huggingface.co/datasets/PlanTL-GOB-ES/SQAC/tree/main
- VeritasQA (COLING 2025). https://aclanthology.org/2025.coling-main.366/
- TruthfulQA-multi-MT. https://huggingface.co/datasets/HiTZ/truthfulqa-multi-MT/blob/main/README.md
- alibi-detect. https://github.com/SeldonIO/alibi-detect
- Evidently, customize data drift. https://docs.evidentlyai.com/metrics/customize_data_drift
- Precios DeepSeek (leido 2026-10-04). https://api-docs.deepseek.com/quick_start/pricing
- transformers, GPT-OSS. https://huggingface.co/docs/transformers/main/en/model_doc/gpt_oss
- Modelos aleatorios de test: https://huggingface.co/hf-internal-testing/tiny-random-GptOssForCausalLM/blob/main/tokenizer.json , https://huggingface.co/hf-internal-testing/tiny-random-OlmoeForCausalLM/blob/main/tokenizer.json , https://huggingface.co/hf-internal-testing/tiny-random-MixtralForCausalLM/blob/main/generation_config.json
- FaithJudge / FaithBench. https://arxiv.org/html/2505.04847v1
