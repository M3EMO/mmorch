---
title: "Colab con GPU: verificador open-weight, looped transformer y señal del enrutador MoE"
created: 2026-10-07
tags: [research, orchestration, colab, gpu, verificador, open-weight, looped-lm, moe, router, confianza-por-token]
status: measured
confidence: "media: una corrida por experimento (aaf con 3 semillas); T4 de Colab Pro, 2026-10-07"
sources: [bd orchestration-pcb, bd orchestration-aaf, scripts/colab_verificador.py (eeb664d), scripts/colab_aaf_looped.py (d4028a4), scripts/colab_moe_senales.py, logs/ablation_results.jsonl, "vault: plan-moe-capa-de-confianza-por-token"]
---
## Contexto

El usuario usa mucho Colab. Se preparó el MCP `colab-mcp` (fijado al commit b9ab389) y la CLI `google-colab-cli` en WSL. Cada corrida usó una T4 con tope total de 20 unidades. Un `trap` apaga la VM siempre, aunque el script falle. Las cuatro corridas gastaron menos de 5 unidades.

## 1. Verificador open-weight (orchestration-pcb)

Mismos 350 ítems pareados de `ablation_paired.py` (seed 42, verdad computada). `Qwen/Qwen3-4B` con razonamiento, servido con vLLM en una T4.

| Verificador | Sensibilidad | Especificidad | Exactitud balanceada | Costo |
|---|---|---|---|---|
| deepseek-reasoner (API) | 1.00 | 1.00 | 1.00 | ~USD 0.05 |
| glm-5.2 (API) | 1.00 | 1.00 | 1.00 | ~USD 0.85 |
| Qwen3-4B (T4) | 1.00 | 0.977 | 0.989 | 0 de API, 40 min de T4 |

- Un modelo de 4B con razonamiento queda al nivel de los verificadores de API.
- Con una GPU a mano, la verificación checkeable puede correr sin API.
- vLLM no corre dentro del kernel de Jupyter, porque pide `sys.stdout.fileno()`. El kernel lo lanza como proceso aparte.
- `colab install vllm` se corta por un timeout interno; la instalación va por `colab exec` con timeout largo.

## 2. Looped transformer chico (orchestration-aaf)

Banco sintético estilo Physics of LLMs: 1000 personas. Las biografías de todas entran al entrenamiento. Las preguntas solo de la mitad. Se mide en la otra mitad: año de nacimiento (almacenamiento) y paridad del año (manipulación, sin cadena de pensamiento).

| Modelo | Parámetros | Almacenamiento | Manipulación (3 semillas) | Media |
|---|---|---|---|---|
| A: 2 capas, 1 vuelta | 0.70 M | ~1.00 | 0.604 / 0.778 / 0.606 | 0.663 |
| B: 2 capas, 4 vueltas | 0.70 M | ~1.00 | 0.658 / 0.704 / 0.680 | 0.681 |
| C: 8 capas, 1 vuelta | 1.89 M | ~0.98 | 0.680 / 0.670 / 0.652 | 0.667 |

- El loop no daña el almacenamiento.
- La ventaja del loop en manipulación (1.8 puntos) queda dentro del ruido entre semillas.
- El spike no confirma "el loop ayuda a manipular" a esta escala. Vueltas fijas, sin PonderNet.

## 3. Señal del enrutador MoE (puerta de la etapa 1 del plan de confianza por token)

Pre-registro: la entropía media del enrutador tiene que predecir el error mejor que la entropía de la salida, por 0.05 de AUROC o más, con menos de 5% de latencia extra. Modelo: `allenai/OLMoE-1B-7B-0125-Instruct` en 4 bits, con el router en fp16.

- Con los 350 problemas de la ablación, OLMoE acertó 3 de 70: demasiado difíciles, sin AUROC posible.
- Con 200 cuentas de 2 y 3 dígitos (verdad computada) acertó 170.

| Señal | AUROC para predecir error |
|---|---|
| Entropía de la salida | 0.806 |
| Entropía media del enrutador | 0.26 (invertida: 0.74) |

- En los errores, el enrutador está más seguro, no menos.
- Ni invertida supera a la salida: la señal no pasa la puerta en OLMoE.
- Pedir `output_router_logits` agrega 2.3% de latencia, dentro del 5%.
- Límite: no se midió la dispersión entre expertos (InnerExpert) ni señales por capa; gpt-oss-20b no entra en una T4.

## Qué cambia

- Para mmorch: un verificador local de 4B es una opción real para lo checkeable cuando hay GPU.
- Para el plan de confianza por token: la entropía media del enrutador no alcanza; la próxima prueba es la dispersión entre expertos o la señal por capa, con el mismo banco de verdad computada.
