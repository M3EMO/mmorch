---
title: Vigia - mercado y competencia en deteccion de alucinaciones y confianza de LLM (2026-10)
mision: Que venden hoy los competidores en deteccion de alucinaciones y confianza de LLM, donde esta el hueco para una capa de confianza por token de una sola pasada sobre MoE autoalojados, y que debe tener Vigia para competir.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://galileo.ai/pricing
  - https://docs.galileo.ai/concepts/luna/luna
  - https://www.splunk.com/en_us/products/luna-evaluation-models.html
  - https://blogs.cisco.com/news/Cisco-announces-the-intent-to-acquire-galileo
  - https://futurumgroup.com/insights/cisco-to-acquire-galileo-ai-agent-observability-cant-run-at-human-speed/
  - https://www.patronus.ai/pricing
  - https://patronus.ai/products
  - https://huggingface.co/PatronusAI/Llama-3-Patronus-Lynx-8B-Instruct
  - https://arxiv.org/abs/2407.08488
  - https://arize.com/pricing/
  - https://github.com/Arize-ai/phoenix
  - https://arize.com/docs/phoenix/evaluation/running-pre-tested-evals/hallucinations
  - https://www.giskard.ai/pricing
  - https://github.com/Giskard-AI/giskard
  - https://cleanlab.ai/tlm/
  - https://help.cleanlab.ai/tlm/
  - https://help.cleanlab.ai/tlm/faq/
  - https://huggingface.co/vectara/hallucination_evaluation_model
  - https://www.vectara.com/pricing
  - https://github.com/guardrails-ai/guardrails
  - https://guardrailsai.com/hub
  - https://docs.nvidia.com/nemo/guardrails/latest/configure-guardrails/guardrail-catalog/fact-checking
  - https://github.com/KRLabsOrg/LettuceDetect
  - https://arxiv.org/abs/2502.17125
  - https://github.com/cvs-health/uqlm
  - https://arxiv.org/abs/2507.06196
  - https://langfuse.com/pricing
  - https://github.com/langfuse/langfuse
  - https://langfuse.com/blog/announcing-acquisition
  - https://wandb.ai/wandb_fc/product-announcements-fc/reports/Humanloop-is-Sunsetting-Migrate-to-Weights-Biases-as-an-alternative--VmlldzoxMzk4ODc1Nw
  - https://www.csoonline.com/article/4058653/check-point-acquires-lakera-to-build-a-unified-ai-security-stack.html
  - https://github.com/whylabs/langkit
  - https://www.geekwire.com/2025/founders-at-seattle-startup-whylabs-join-apple-following-under-the-radar-acquisition
  - https://raw.githubusercontent.com/haizelabs/verdict/main/README.md
  - https://www.beri.net/article/beacon-haize-labs-acquisition-ai-red-teaming-vendor-continuity-export
  - https://www.aimon.ai/posts/aimon-hdm-2-hallucination-detection-model
  - https://arxiv.org/abs/2504.07069
  - https://arxiv.org/abs/2608.17687
  - https://arxiv.org/abs/2406.15927
  - https://arxiv.org/abs/2407.07071
  - https://proceedings.neurips.cc/paper_files/paper/2024/hash/3c1e1fdf305195cd620c118aaa9717ad-Abstract.html
  - https://arxiv.org/abs/2607.11414
  - https://arxiv.org/abs/2605.28571
  - https://arxiv.org/abs/2504.11975
  - https://portal.odesia.uned.es/en/dataset/mu-shroom-2025-es
  - https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
  - https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
  - https://lmsysorg.mintlify.app/docs/basic_usage/openai_api_completions
  - https://huggingface.co/docs/transformers/model_doc/gpt_oss
  - https://huggingface.co/openai/gpt-oss-20b
  - https://huggingface.co/openai/gpt-oss-20b/raw/main/config.json
  - https://iapp.org/news/a/novedades-legislativas-en-argentina-sobre-protecci-n-de-datos-personales-e-inteligencia-artificial
  - https://www4.hcdn.gob.ar/dependencias/dsecretaria/Periodo2025/PDF2025/TP2025/3540-D-2025.pdf
created: 2026-10-04
---

## Tronco

Ningun competidor vende hoy confianza por token de una sola pasada a partir de senales internas de MoE autoalojados, en espanol y dentro de la infraestructura del cliente; el mercado se divide entre plataformas de observabilidad con juez LLM a nivel traza y detectores que son un segundo modelo, y Vigia ocupa el hueco entre ambos si resuelve la extraccion del router en el servidor de inferencia.

## Hallazgos

### 1. Mapa del mercado en cuatro grupos

El mercado se agrupa en cuatro familias de producto. Cada familia resuelve un problema distinto.

| Grupo | Que hace | Ejemplos | Senal usada | Granularidad tipica |
|---|---|---|---|---|
| A. Observabilidad y evals | Trazas, datasets, experimentos, juez LLM | Langfuse, Arize Phoenix/AX, Galileo, Patronus (Percival) | Juez LLM externo sobre texto | Respuesta, traza, sesion |
| B. Detector como segundo modelo | Un modelo chico verifica la respuesta contra el contexto | Lynx, HHEM, LettuceDetect, AIMon HDM-2, Luna-2, AlignScore | Texto (contexto + respuesta) | Respuesta; span en LettuceDetect y HDM-2 |
| C. Guardrails en runtime | Validan entrada y salida, aplican accion | Guardrails AI, NeMo Guardrails, Lakera Guard | Validadores, NLI, juez, resampleo | Respuesta u oracion |
| D. Incertidumbre caja blanca/negra | Puntaje de confianza desde logprobs o consistencia | Cleanlab TLM, UQLM, LangKit | Logprobs, resampleo, autorreflexion | Respuesta; claim en UQLM |

Ningun producto comercial verificado usa senales del router MoE. Las senales internas (hidden states, atencion, router) solo aparecen en papers academicos.

### 2. Ficha por competidor

Precios con fecha de lectura 2026-10-04 salvo indicacion.

| Producto | Metodo | Deployment | Precio publicado | Licencia OSS | Estado corporativo |
|---|---|---|---|---|---|
| Galileo (Luna-2) | SLM evaluadores 3B/8B fine-tuneados; metricas de alucinacion, toxicidad, tool selection | SaaS; Enterprise: hosted, VPC u on-prem; Luna-2 self-host exige GPU L4 o superior | Free USD 0 (5.000 trazas/mes); Pro USD 100/mes facturado anual (50.000 trazas/mes); Enterprise a medida. Luna-2 solo en Enterprise. Luna-2: USD 0,02 por millon de tokens en docs de Galileo y USD 0,12 en pagina de Splunk (discrepancia) | Cerrado | Cisco anuncio la compra el 2026-04-09 y la completo el 2026-05-22; galileo.ai/luna-2 redirige a Splunk |
| Patronus AI (Lynx) | LLM juez fine-tuneado 8B/70B; salida JSON con REASONING y SCORE PASS/FAIL | API SaaS; Enterprise on-prem o VPC dedicada | Developer gratis con USD 10 de credito; evaluador chico USD 10 por 1.000 llamadas; evaluador grande USD 20 por 1.000; explicaciones USD 10 por 1.000 | Pesos Lynx en CC-BY-NC-4.0 (no comercial) | Pivot a agentes (Percival) y "world models"; ronda Serie B USD 50M en 2026-06 [no verificado en fuente primaria] |
| Arize Phoenix / AX | Juez LLM con plantilla; etiqueta "hallucinated"/"grounded", score 1.0/0.0, explicacion; nivel span, traza y sesion | Phoenix self-host (Docker, Helm); AX SaaS; Enterprise SaaS o self-host | AX Free (25k spans/mes, 15 dias); AX Pro USD 50/mes (50k spans, 30 dias); Enterprise a medida | Phoenix bajo Elastic License 2.0 (no OSI); 11,7k estrellas | Independiente |
| Giskard | Escaneo adversarial (50+ probes), red teaming continuo, quality_scan para RAG (sucesor de RAGET) | Libreria local; Enterprise on-prem, nube privada o SaaS | Libreria USD 0; Enterprise a medida | Apache 2.0; v3 reescrita; requiere Python 3.12+ | Independiente |
| Cleanlab TLM | Autorreflexion + consistencia (multiples respuestas) + probabilidades de token + metodos propietarios | API SaaS; VPC en la nube del cliente | Plan pay-per-token; tarifas no publicas (visibles dentro de la cuenta). La pagina /pricing devuelve 404 | Cerrado | Independiente |
| Vectara (HHEM) | Clasificador NLI sobre flan-t5-base (~0,1B); score 0..1 de consistencia factual | HHEM-2.1-Open local; HHEM-2.3 dentro de la plataforma | Plataforma: SaaS desde USD 100K/ano; VPC desde USD 250K/ano; on-prem desde USD 500K/ano | HHEM-2.1-Open Apache 2.0 (solo ingles) | Independiente |
| Guardrails AI | Validadores del Hub (Provenance Embeddings, Provenance LLM, Provenance NLI, Wiki Provenance); acciones on-fail: exception, reask, fix, filter, noop | Libreria y modo servidor REST | OSS gratis; Guardrails Pro a medida; USD 50.000/ano en AWS Marketplace [no verificado en fuente primaria] | Apache 2.0; 7,5k estrellas | Independiente |
| NVIDIA NeMo Guardrails | Rails: self check facts (juez), self check hallucination (resampleo, 2 respuestas extra por defecto), AlignScore (RoBERTa), Lynx | Self-host; AlignScore y Lynx requieren endpoint aparte | Gratis | Apache 2.0 [licencia no verificada en esta sesion] | NVIDIA |
| LettuceDetect | Clasificacion por token (ModernBERT, EuroBERT, mmBERT, Qwen-2B en v2); agrega a spans por caracter | Local, CPU soportado; API FastAPI | Gratis | MIT; v0.2.2 del 2026-07-05 | Proyecto KRLabs |
| UQLM (CVS Health) | Scorers caja negra, caja blanca (min prob, prob normalizada, negentropia top-K, margen), juez y ensemble; claim-level con LongTextUQ | Libreria local | Gratis | Apache 2.0 | CVS Health |
| Langfuse | Trazas, prompts, datasets, evals con juez LLM, anotacion | Cloud o self-host (Postgres, ClickHouse, Redis, S3) | Hobby USD 0 (50k unidades, 30 dias); Core USD 29/mes; Pro USD 199/mes; Enterprise USD 2.499/mes; excedente USD 8 por 100k unidades | MIT nucleo + carpeta ee; 35,4k estrellas | ClickHouse lo compro el 2026-01-16 |
| Humanloop | Plataforma de prompts y evals | Ya no existe | - | - | Cerro el 2025-09-08; el equipo paso a Anthropic |
| Lakera Guard | Seguridad: prompt injection, fuga de datos, moderacion; "salida desalineada" [no verificado] | SaaS; Enterprise | Community gratis; Enterprise a medida [fuente secundaria] | Cerrado | Check Point anuncio la compra por ~USD 300M en 2025 |
| WhyLabs LangKit | Metricas de texto; alucinacion por consistencia entre respuestas | Libreria local | Gratis | Apache 2.0 | Apple adquirio WhyLabs en 2025; plataforma liberada como OSS |
| Haize Labs | Red teaming automatico (Cascade), framework de jueces Verdict | SaaS | No publicado | Verdict OSS | Beacon Software la compro el 2026-09-17 sin compromiso publico de continuidad |
| AIMon HDM-2 | Qwen-2.5-3B + LoRA + cabezas de clasificacion; score de documento y scores por token | SaaS y version abierta 3B | No verificado | Pesos 3B abiertos [licencia no verificada] | Independiente |

### 3. Metricas publicadas de los detectores

Las cifras vienen de cada vendedor o paper. No son comparables entre si porque usan datasets distintos.

| Detector | Metrica publicada | Dataset | Costo de inferencia |
|---|---|---|---|
| InnerExpert (paper base de Vigia) | AUROC hasta 0,91 por respuesta y 0,76 por token; una sola pasada | 5 datasets, 2 arquitecturas MoE | Casi cero: reusa la pasada del generador |
| Lynx 8B / 70B | 82,9% / 87,4% de accuracy | HaluBench | Segundo LLM de 8B o 70B |
| HHEM-2.1-Open | 74,28% balanced accuracy | RAGTruth-QA | ~1,5 s por 2k tokens en CPU, <600 MB RAM |
| LettuceDetect (paper) | F1 79,22% a nivel ejemplo | RAGTruth | Encoder 150M-610M |
| LettuceDetect v2 | Span-F1 0,689 (Qwen-2B), 0,642 (mmBERT) | Test unificado v2 | Encoder o 2B |
| Luna-2 | F1 0,95; 152 ms promedio | Interno Galileo | GPU L4+ |
| Cleanlab TLM | AUROC 0,91 contra 0,78 del juez LLM | Interno Cleanlab | Varias llamadas LLM; "desde 300 ms" |
| AIMon HDM-2 | Supera GPT-4o-mini en RAGTruth (sin cifra leida) | RAGTruth, TruthfulQA, HDM-Bench | 500 ms en una GPU L4 |
| Semantic Entropy Probes | AUROC 0,7 a 0,95 segun escenario | Varios | Sonda lineal sobre hidden states |
| Probes en finanzas (2607.11414) | AUROC 0,68-0,77 en respuestas confiadas contra 0,55-0,63 de logprobs | FinQA, TAT-QA | Sonda lineal; solo ingles |

Dato clave del paper de finanzas: entre respuestas "confiadas" (8 de 8 resampleos coinciden), entre 15% y 23% estan mal en FinQA. La consistencia y los logprobs no detectan esos errores. Las senales internas si los separan mejor.

### 4. Senales internas: estado de la investigacion

- InnerExpert (arXiv 2608.17687, 2026-08-18) usa entropia del router, desacuerdo entre expertos y patrones de uso de expertos.
- InnerExpert combina esas senales con senales estandar del transformer en un vector por token.
- InnerExpert etiqueta con un pipeline de juez LLM, sin anotacion manual.
- El abstract no nombra los modelos MoE ni los datasets [no verificado: nombres concretos].
- Semantic Entropy Probes aproxima la entropia semantica desde hidden states sin resamplear.
- Lookback Lens usa la razon de atencion contexto/generado y reduce alucinacion 9,6% en XSum con decodificacion guiada.
- LLM-Check (NeurIPS 2024) reporta aceleraciones de hasta 45x y 450x sobre baselines.
- Ninguno de estos metodos tiene producto comercial verificado.

### 5. Viabilidad tecnica de extraer el router (riesgo del plan)

| Pila | Que expone | Limitacion |
|---|---|---|
| HF transformers | `output_router_logits=True` devuelve `router_logits` por capa, forma (batch, seq, num_experts) | La doc dice que "no deberian devolverse durante inferencia"; uso orientado a la loss auxiliar; la doc tiene inconsistencia pre/post-softmax |
| vLLM | `--enable-return-routed-experts` devuelve IDs de expertos int16, forma [gen_len, num_moe_layers, top_k] | Solo IDs, no logits ni probabilidades; datos solo al terminar el request (sin streaming); ~2% menos throughput; solo engine V1; incompatible con async scheduling |
| SGLang | `return_routed_experts` devuelve IDs int32 en base64, forma [num_tokens, num_layers, top_k] | Solo IDs; pensado para RL |

Conclusion tecnica: los IDs de expertos alcanzan para senales de "uso de expertos". No alcanzan para entropia del router. La entropia exige los logits completos. Vigia necesita un hook propio dentro del servidor de inferencia o un plugin de vLLM.

Datos de gpt-oss-20b leidos de config.json:

- 24 capas, 32 expertos locales, 4 expertos por token, hidden_size 2880.
- El router queda fuera de la cuantizacion MXFP4 (`model.layers.*.mlp.router` en `modules_to_not_convert`).
- Consecuencia: los logits del router conservan precision BF16 aun con el modelo cuantizado.
- gpt-oss-120b usa 128 expertos y 4 activos por token, segun la doc de transformers.
- gpt-oss-20b corre en 16 GB de memoria segun el model card. No entra en la laptop de build de 8 GB.

### 6. Como presentan resultados en la UI

| Producto | Presentacion | Verificado |
|---|---|---|
| Arize Phoenix | Etiqueta + score + explicacion del juez, por span/traza/sesion | Salida verificada; diseno visual no verificado |
| Patronus Lynx | PASS/FAIL + razonamiento en vinetas | Verificado |
| Luna-2 | Veredicto deterministico de un token | Verificado |
| Vectara HHEM | Score 0..1 de consistencia factual por respuesta | Verificado |
| LettuceDetect | Demo Streamlit con spans alucinados resaltados | Verificado |
| UQLM | DataFrame con `.to_df()`; sin dashboard | Verificado |
| Giskard | Reporte de escaneo con `.print_report()` y vistas del Hub | Parcial |
| Langfuse | Scores numericos y categoricos adjuntos a trazas; colas de anotacion | Parcial |
| Herramientas de logprobs (Transformer Lab, eli5, LLMbench) | Heatmap por token con tooltip de probabilidad | Verificado por busqueda |

Hallazgo de diseno (arXiv 2605.28571, ACM FAccT 2026, n=192, preguntas medicas):

- Mostrar incertidumbre por token aumento el acuerdo de los usuarios con la IA.
- Mostrar incertidumbre por respuesta no aumento el acuerdo y bajo la confianza propia del usuario.
- Mostrar incertidumbre por paso de razonamiento redujo la verificacion externa.
- Conclusion: el resaltado por token puede generar sobreconfianza. La UI de Vigia debe forzar una accion, no solo colorear.

### 7. Recursos en espanol

- Mu-SHROOM (SemEval 2025 Task 3) etiqueta alucinaciones a nivel span en 14 idiomas, espanol incluido.
- El set en espanol tiene 200 unidades: 50 de desarrollo y 150 de test.
- LettuceDetect ofrece modelos EuroBERT con espanol.
- HHEM-2.1-Open solo cubre ingles. HHEM-2.3 comercial declara soporte cross-lingual.
- Ningun competidor publica metricas en espanol rioplatense ni en dominios regulados de LatAm.

### 8. Contexto regulatorio Argentina

- Hay proyectos de reforma integral de la Ley 25.326 de datos personales inspirados en el GDPR y la LGPD.
- El proyecto 3540-D-2025 propone transparencia algoritmica, derecho a explicacion y auditoria de sistemas automatizados.
- Los proyectos incluyen oposicion a decisiones automatizadas con efectos juridicos.
- Ninguno esta sancionado segun las fuentes leidas [estado parlamentario a 2026-10 no verificado].

## Implicancias para Vigia

### Hueco de mercado

- El grupo A cobra por traza y usa un juez LLM externo. Agrega latencia y costo por llamada, y envia datos a un tercero.
- El grupo B exige un segundo modelo y casi siempre un contexto RAG. No sirve para preguntas sin contexto recuperado.
- El grupo D usa logprobs o resampleo. El paper de finanzas muestra que los logprobs fallan en respuestas confiadas.
- Ningun producto usa senales del router MoE. Ningun producto ofrece confianza por token sin segundo modelo.
- Ningun producto apunta a espanol y a infraestructura on-prem en LatAm con precio accesible. Vectara on-prem arranca en USD 500K/ano.
- La consolidacion (Cisco-Galileo, ClickHouse-Langfuse, Check Point-Lakera, Beacon-Haize, Anthropic-Humanloop, Apple-WhyLabs) deja clientes medianos sin proveedor independiente.

### Features table-stakes (Vigia DEBE tenerlas)

1. Libreria Python `pip install` con API de una linea para envolver un modelo HF MoE y devolver scores por token.
2. Puntaje por token y agregado por span y por respuesta, con umbral configurable.
3. Exportacion OpenTelemetry/OpenInference para que los scores entren a Langfuse y Phoenix como atributos de span.
4. Integracion directa: push de scores a Langfuse via su API de scores.
5. API REST autoalojable con Docker Compose.
6. Panel web con resaltado de spans dudosos y tooltip con las senales que contribuyen.
7. Reporte de evaluacion: AUROC por token y por respuesta, curva ROC, diagrama de calibracion y sobrecarga de latencia en porcentaje.
8. Acciones por fragmento dudoso: revision humana, re-consulta con RAG, abstencion. Equivale a las acciones on-fail de Guardrails.
9. Cola de revision humana con etiquetado que realimente la calibracion.
10. Datasets versionados de calibracion con import/export JSONL.
11. RBAC, SSO y log de auditoria en el tier pago.
12. Retencion configurable y redaccion de PII antes de persistir texto.
13. Wrapper compatible con la API de chat de OpenAI para adopcion sin cambiar el cliente.

### Debilidades de la competencia que Vigia mejora

| Debilidad observada | Competidor | Mejora concreta de Vigia |
|---|---|---|
| Segundo modelo con latencia propia (152 ms a 1,5 s) | Luna-2, HHEM, Lynx, HDM-2 | Reusar la pasada del generador; meta < 5% de latencia agregada, medida y publicada en el reporte |
| Requiere contexto RAG para verificar | Lynx, HHEM, LettuceDetect, AlignScore | Senal intrinseca del modelo; funciona sin contexto |
| Datos salen a un tercero | Cleanlab API, Patronus API, Galileo SaaS | Todo corre en el cluster del cliente; cero llamadas externas en produccion |
| Granularidad de respuesta o traza | Phoenix, Langfuse, Galileo, TLM | Granularidad por token con agregacion a span |
| Pesos no comerciales | Lynx (CC-BY-NC-4.0) | Libreria Apache 2.0, alineada con la licencia de gpt-oss |
| Licencia no OSI | Phoenix (ELv2) | Nucleo bajo licencia OSI |
| Solo ingles | HHEM-2.1-Open, paper de finanzas | Calibracion y benchmark en espanol (Mu-SHROOM-es + set propio) |
| Precio enterprise inalcanzable para LatAm | Vectara (USD 100K-500K/ano) | Tier medio en USD con calibracion por cliente |
| Sin UI | UQLM, LettuceDetect (solo demo) | Panel con flujo de accion |
| Resaltado que induce sobreconfianza | Heatmaps de logprobs | Resaltar solo lo dudoso; exigir accion; no pintar de verde lo "seguro" |
| Riesgo de continuidad por adquisicion | Haize, Humanloop, WhyLabs | Nucleo abierto que el cliente puede operar solo |

### Decisiones de diseno recomendadas

1. Construir un extractor con hooks de PyTorch sobre el modulo `mlp.router` de cada capa, no sobre `output_router_logits`. El hook captura logits completos en inferencia.
2. Entregar dos backends de extraccion: HF transformers (referencia y tests) y plugin de vLLM (produccion). Documentar que el flag nativo de vLLM solo da IDs.
3. Definir un contrato de features por token versionado: entropia del router por capa, margen top-1/top-2, desacuerdo entre expertos, IDs de expertos, logprob y entropia del token.
4. Degradar con elegancia: si el backend solo entrega IDs (vLLM nativo, SGLang), calcular solo features de uso de expertos y marcar el score como "modo reducido".
5. Usar un detector liviano (regresion logistica o GBDT chico) entrenable en CPU. Las pruebas deben correr en CPU con un MoE diminuto (por ejemplo OLMoE o un MoE sintetico aleatorio).
6. Probar el pipeline en la laptop de 8 GB con un MoE sintetico. Reservar gpt-oss-20b para un entorno con 16 GB o mas.
7. Exigir un juez de calibracion con licencia comercial. Excluir Lynx por CC-BY-NC. Preferir un juez local (por ejemplo gpt-oss-120b) cuando los datos son sensibles.
8. Calibrar umbrales por accion: un umbral alto para abstencion y uno bajo para revision humana, porque AUROC 0,76 por token implica falsos positivos.
9. Agregar tokens subword a palabras y spans antes de mostrar. El usuario no debe ver fragmentos de subword.
10. Puntuar solo el canal "final" del formato harmony de gpt-oss. Tratar el canal de razonamiento como opcional y separado.
11. Mostrar en el panel el modelo, la version de cuantizacion y la fecha de calibracion junto a cada score.
12. Licenciar el nucleo bajo Apache 2.0 y el panel enterprise en una carpeta `ee` con licencia comercial, como Langfuse.

### Packaging y pricing open-core recomendado

Las referencias publicadas fijan el rango. Langfuse Core cuesta USD 29/mes, Arize Pro USD 50/mes y Galileo Pro USD 100/mes. Vectara arranca en USD 100K/ano. Las cifras de Vigia de abajo son hipotesis a validar con entrevistas, no datos de mercado.

| Tier | Contenido | Precio sugerido [hipotesis] |
|---|---|---|
| Community (Apache 2.0) | Extractor, detector, CLI, reporte AUROC/latencia, export OTel, detector generico pre-entrenado | Gratis |
| Team (self-host) | Panel web, cola de revision, acciones por fragmento, 1 modelo, hasta 10 usuarios | USD 300-600/mes por despliegue de modelo |
| Enterprise (self-host) | SSO, RBAC, auditoria, multi-modelo, multi-tenant, retencion y PII, SLA | Licencia anual desde USD 15-30K |
| Calibracion por cliente | Etiquetado con juez sobre datos del cliente, entrenamiento del detector, reporte firmado | Pago unico USD 3-8K por modelo y dominio; recalibracion trimestral opcional |
| Soporte | Soporte en espanol, horario LatAm | Incluido en Enterprise; adicional en Team |

Recomendacion de packaging:

- Cobrar por despliegue de modelo, no por traza. El costo marginal de Vigia por token es casi cero, y cobrar por traza copia el modelo de Galileo y Langfuse sin justificacion.
- Vender la calibracion como servicio separado. Es el entregable que el cliente no puede replicar con la libreria sola.
- Publicar los precios en la web. Cleanlab, Lakera, Guardrails Pro y Vectara Enterprise los ocultan, y eso frena a compradores medianos.

## Edge cases y riesgos

- vLLM nativo no devuelve logits del router. Sin plugin propio, la entropia del router no existe en produccion.
- vLLM entrega el routing solo al terminar el request. El resaltado en vivo durante streaming requiere hook propio.
- vLLM pierde el routing acumulado de requests preemptados sin aviso. El extractor debe detectar arrays faltantes.
- Expert parallelism y tensor parallelism reparten expertos entre GPUs. El extractor debe juntar logits de varias GPUs.
- La decodificacion especulativa (MTP) cambia que pasadas generan cada token. El score puede quedar desalineado con el texto final.
- Otros MoE cuantizados (GGUF, AWQ) pueden cuantizar el router. Eso cambia la distribucion de la entropia y obliga a recalibrar.
- La doc de transformers describe `router_logits` a la vez como "raw" y "post-softmax". El extractor debe verificar la escala con un test.
- En gpt-oss el top-k se aplica antes del softmax [no verificado en el codigo]. La entropia sobre los 4 elegidos difiere de la entropia sobre los 32.
- Un cambio de version del modelo, de cuantizacion o de plantilla de chat invalida la calibracion. El panel debe alertar ese drift.
- El juez de calibracion puede tener sesgo en espanol. Hace falta un subconjunto etiquetado por humanos para auditar al juez.
- El desbalance de clases (pocos tokens alucinados) infla la accuracy. Reportar AUROC, AUPRC y F1 por clase.
- AUROC 0,76 por token implica falsos positivos frecuentes. Un umbral mal puesto satura la cola de revision humana.
- El resaltado por token puede aumentar la sobreconfianza (FAccT 2026). El texto no resaltado no es "verificado".
- El panel guarda prompts y respuestas con datos sensibles. Requiere redaccion de PII, cifrado y retencion configurable.
- Multi-tenant: un cliente nunca debe ver datos ni calibraciones de otro. Requiere aislamiento por tenant en base de datos y en almacenamiento de detectores.
- Lynx no sirve como juez comercial por su licencia CC-BY-NC-4.0.
- Galileo, Patronus o Cleanlab pueden agregar senales internas. Cisco tiene recursos para hacerlo rapido.
- vLLM o SGLang pueden exponer logits del router en una version futura. Eso reduce la barrera tecnica de Vigia.
- gpt-oss-20b necesita 16 GB. La maquina de build tiene 8 GB y no puede correr el modelo objetivo.

## Objeciones

- No pude leer precios por token de Cleanlab TLM. La pagina /pricing da 404 y la FAQ remite a la cuenta.
- No pude leer la pagina de precios de Lakera. El dato de tiers viene de una fuente secundaria.
- El precio de Guardrails Pro en AWS Marketplace viene de un agregador, no de AWS.
- Luna-2 tiene dos precios publicados (USD 0,02 y USD 0,12 por millon de tokens). No pude resolver cual rige tras la compra de Cisco.
- No verifique capturas de pantalla de las UIs de Galileo, Phoenix ni Langfuse. La seccion de UI se basa en documentacion.
- El abstract de InnerExpert no nombra modelos ni datasets. Falta leer el PDF completo para saber si probo gpt-oss.
- No hay evidencia publicada de que las senales del router funcionen en espanol. Es la hipotesis central sin medir.
- La meta de < 5% de latencia no esta medida en ningun backend de produccion. vLLM reporta ~2% solo para devolver IDs.
- El hallazgo de sobreconfianza viene de un estudio medico con 192 participantes. Falta validarlo en finanzas y legal.
- Los precios sugeridos para Vigia son hipotesis. Falta discovery con equipos de IA en empresas medianas de Argentina.
- No verifique el estado parlamentario de la reforma de la Ley 25.326 a octubre de 2026.
- No verifique la licencia de NeMo Guardrails ni la de los pesos abiertos de HDM-2 en esta sesion.

## Fuentes

Fuentes primarias leidas (pagina abierta con WebFetch):

- Galileo pricing: https://galileo.ai/pricing
- Galileo Luna-2 docs: https://docs.galileo.ai/concepts/luna/luna
- Splunk Luna: https://www.splunk.com/en_us/products/luna-evaluation-models.html
- Patronus pricing: https://www.patronus.ai/pricing
- Patronus products: https://patronus.ai/products
- Lynx model card: https://huggingface.co/PatronusAI/Llama-3-Patronus-Lynx-8B-Instruct
- Arize pricing: https://arize.com/pricing/
- Phoenix repo: https://github.com/Arize-ai/phoenix
- Phoenix hallucination eval: https://arize.com/docs/phoenix/evaluation/running-pre-tested-evals/hallucinations
- Giskard pricing: https://www.giskard.ai/pricing
- Giskard repo: https://github.com/Giskard-AI/giskard
- Cleanlab TLM: https://cleanlab.ai/tlm/ , https://help.cleanlab.ai/tlm/ , https://help.cleanlab.ai/tlm/faq/
- HHEM model card: https://huggingface.co/vectara/hallucination_evaluation_model
- Vectara pricing: https://www.vectara.com/pricing
- Guardrails repo: https://github.com/guardrails-ai/guardrails
- Guardrails Hub: https://guardrailsai.com/hub
- NeMo Guardrails fact-checking: https://docs.nvidia.com/nemo/guardrails/latest/configure-guardrails/guardrail-catalog/fact-checking
- LettuceDetect repo: https://github.com/KRLabsOrg/LettuceDetect
- UQLM repo: https://github.com/cvs-health/uqlm
- Langfuse pricing: https://langfuse.com/pricing
- Langfuse repo: https://github.com/langfuse/langfuse
- LangKit repo: https://github.com/whylabs/langkit
- Beacon/Haize: https://www.beri.net/article/beacon-haize-labs-acquisition-ai-red-teaming-vendor-continuity-export
- InnerExpert: https://arxiv.org/abs/2608.17687
- Probes en finanzas: https://arxiv.org/abs/2607.11414
- Granularidad de incertidumbre (FAccT 2026): https://arxiv.org/abs/2605.28571
- vLLM routed experts replay: https://docs.vllm.ai/en/v0.21.0/training/routed_experts_replay/
- vLLM foro: https://discuss.vllm.ai/t/can-vllm-return-expert-selection-info-to-support-routing-replay/1742
- transformers gpt-oss: https://huggingface.co/docs/transformers/model_doc/gpt_oss
- gpt-oss-20b model card y config: https://huggingface.co/openai/gpt-oss-20b , https://huggingface.co/openai/gpt-oss-20b/raw/main/config.json

Fuentes vistas solo como resultado de busqueda (resumen del buscador, no abiertas):

- Cisco-Galileo: https://blogs.cisco.com/news/Cisco-announces-the-intent-to-acquire-galileo , https://futurumgroup.com/insights/cisco-to-acquire-galileo-ai-agent-observability-cant-run-at-human-speed/
- Lynx paper: https://arxiv.org/abs/2407.08488
- LettuceDetect paper: https://arxiv.org/abs/2502.17125
- UQLM paper: https://arxiv.org/abs/2507.06196
- ClickHouse-Langfuse: https://langfuse.com/blog/announcing-acquisition
- Humanloop sunset: https://wandb.ai/wandb_fc/product-announcements-fc/reports/Humanloop-is-Sunsetting-Migrate-to-Weights-Biases-as-an-alternative--VmlldzoxMzk4ODc1Nw
- Check Point-Lakera: https://www.csoonline.com/article/4058653/check-point-acquires-lakera-to-build-a-unified-ai-security-stack.html
- Apple-WhyLabs: https://www.geekwire.com/2025/founders-at-seattle-startup-whylabs-join-apple-following-under-the-radar-acquisition
- Verdict: https://raw.githubusercontent.com/haizelabs/verdict/main/README.md
- AIMon HDM-2: https://www.aimon.ai/posts/aimon-hdm-2-hallucination-detection-model , https://arxiv.org/abs/2504.07069
- Semantic Entropy Probes: https://arxiv.org/abs/2406.15927
- Lookback Lens: https://arxiv.org/abs/2407.07071
- LLM-Check: https://proceedings.neurips.cc/paper_files/paper/2024/hash/3c1e1fdf305195cd620c118aaa9717ad-Abstract.html
- Mu-SHROOM: https://arxiv.org/abs/2504.11975 , https://portal.odesia.uned.es/en/dataset/mu-shroom-2025-es
- SGLang routed experts: https://lmsysorg.mintlify.app/docs/basic_usage/openai_api_completions
- Reforma Ley 25.326: https://iapp.org/news/a/novedades-legislativas-en-argentina-sobre-protecci-n-de-datos-personales-e-inteligencia-artificial , https://www4.hcdn.gob.ar/dependencias/dsecretaria/Periodo2025/PDF2025/TP2025/3540-D-2025.pdf
