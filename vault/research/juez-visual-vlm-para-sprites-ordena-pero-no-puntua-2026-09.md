---
title: Juez visual VLM para sprites: ordena pero no puntua (2026-09)
created: 2026-09-17
tags: [research, mmorch, vlm, juez, sprites, pixel-art, sdlc, oraculo]
status: verified
confidence: 0.8
sources: [https://arxiv.org/abs/2604.25235, https://arxiv.org/abs/2402.04788, https://arxiv.org/abs/2604.21523, https://arxiv.org/abs/2604.19966, https://arxiv.org/abs/2603.06148, https://arxiv.org/abs/2508.03789, https://github.com/Li-Mingshuang/pixellint]
---
Investigacion para el ticket 15 del mapa sdlc-6-gates (oraculo visual del pipeline SDLC). Dos busquedas web con
subagentes + un prototipo medido en la maquina del usuario.

## Lo medido por otros

- Un juez VLM ORDENA razonablemente y PUNTUA mal. gemini-2.5-flash: Pearson 0.459, Spearman 0.446, 32.1% de acierto
  exacto; en estetica con tolerancia +-1 llega a 88.3% (arXiv 2604.25235, 2026-04-28).
- MLLM-as-a-Judge (arXiv 2402.04788, ICML 2024): GPT-4V acuerda 70% scoring / 79.3% pares / 69% ranking; Gemini
  67.7% / 72% / 47% con Pearson 0.262. El modo PARES siempre gana al scoring.
- Caso favorable: realismo de imagenes aumentadas con gemini-2.5-flash contra voto de 3 humanos, kappa 0.738 y
  acierto 0.880 (arXiv 2603.04325).
- Un reward model chico sin LLM iguala o supera al juez VLM: HPSv2 83.3% en dominio y 65.3% fuera (HPSv3,
  arXiv 2508.03789).
- Modos de falla: ceguera a degradaciones (pairwise 13-18%, scoring 32-54%, arXiv 2604.21523); defectos finos
  (67% blur, 66% ruido, <27% artefactos de denoising, DistortBench 2604.19966); caida por resolucion (59.63% a 384px
  vs 67.96% a 1536px, VLM-RobustBench 2603.06148); sesgo de posicion (88.2% repite el orden) y de verbosidad.
- Few-shot: 5 ejemplos por criterio suben 5.1 a 11.8 puntos en 3 de 4 jueces (arXiv 2605.24737). Reemplazar a un
  humano pide 50 a 100 items con 2-3 anotadores (alt-test, ACL 2025).
- Pixel art: CERO benchmarks de juez VLM. No hay dataset publico de sprites etiquetados por calidad.
- Herramientas deterministas: pixellint (Python, OSS, exit code), Aseprite CLI headless con metadata JSON, ImageMagick
  `compare -metric AE`. En regresion visual se falla recien sobre 0.01% de pixeles distintos.

## Lo medido aca

Prototipo con Pillow + numpy: 10 defectos inyectados en un sprite sintetico, 10 atrapados, 0 falsos positivos
(tamano, paleta cerrada, tope de colores, alfa binario, pixel huerfano, borde sucio, hitbox fuera del alfa, drift
entre frames, paleta inestable, loop abierto). Nadie publica el chequeo de hitbox contra alfa; salio de aca.

## Decision que sostiene (mmorch)

`mmorch/sprites.py`: la capa determinista BLOQUEA en la etapa 5; el juez visual pregunta de a pares contra una
referencia aprobada mas una rubrica binaria, nunca puntaje absoluto, y queda EN SOMBRA hasta 50 sprites etiquetados
por el humano con kappa >= 0.6. El sprite viaja agrandado x8 con vecino-mas-cercano por la caida de acierto en
imagenes chicas. El endpoint OpenAI-compatible de Gemini acepta `image_url`, asi que no hizo falta tocar el proveedor.
