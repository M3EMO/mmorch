---
title: "Plan MoE (capa de confianza por token): qué sirve a mmorch y qué le da mmorch al plan"
created: 2026-10-04
tags: [research, orchestration, moe, confianza, alucinacion, verificacion, emprendimiento, cuantizacion]
status: verdict
confidence: "media: lectura del plan del usuario del 2026-10-04, sin experimentos"
sources: ["Plan de proyecto: de la investigación MoE a un emprendimiento (Mateo P", 2026-10-04); Fonseca, Rodrigues y Romano (2026), Mixture-of-Expert Blocks Contain Strong Hallucination Detection Signals (InnerExpert)]
---
## El plan

Producto: una capa de confianza por token para modelos MoE autoalojados (por ejemplo gpt-oss-20b y gpt-oss-120b). Usa señales que el modelo ya calcula: la entropía del enrutador y la dispersión entre expertos, como InnerExpert (AUROC 0.76 por token). La puerta de la etapa 1 pide que la señal supere a la entropía de la salida con menos de 5% de latencia extra.

## Qué le sirve a mmorch: casi nada directo

- mmorch usa modelos por API (DeepSeek, GLM); una API no expone los logits del enrutador.
- Sin pesos locales no hay señal interna; el plan mismo lo marca como límite del mercado.
- La cuantización de tareas de SDLC mide archivos y acoplamiento de una feature; la señal del plan mide la duda de cada token. Son problemas distintos.
- Lo más cercano en caja negra: la entropía de la salida con logprobs, que es justo la línea base que el plan quiere superar.

## Qué le da mmorch al plan

- Etiquetas sin juez: el plan etiqueta la calibración con un juez LLM una sola vez. mmorch midió que un juez LLM refuta mucho trabajo correcto en casos difíciles. Los conjuntos con verdad computada de mmorch (por ejemplo 350 ítems aritméticos) dan etiquetas de alucinación sin juez para la primera calibración.
- Método de banco: pre-registro, oráculo computable, bootstrap por ítem y revisión adversarial antes de congelar. Sirve tal cual para la puerta de la etapa 1.
- Desacuerdo de caja negra: el desacuerdo entre modelos o entre muestras es la versión sin pesos del desacuerdo entre expertos. mmorch ya lo usa para escalar a un verificador solo cuando hay duda.

## Críticas al plan

- Un AUROC de 0.76 por token puede no alcanzar para un cliente exigente; la puerta de la etapa 1 es la decisión correcta.
- vLLM no expone los logits del enrutador por defecto: el riesgo de integración es real y conviene medirlo primero.
- La ventaja es de ejecución (calibración por cliente, español, soporte local), no de idea: un proveedor grande puede copiar la función.
