---
title: Surcos y conectividad funcional (Häkkinen 2025) no sirven para entrenar LLM ni para mmorch
created: 2026-10-02
tags: [research, orchestration, paper, neurociencia, entrenamiento, metodologia, modelo-nulo]
status: verdict
confidence: alta para "no aplica"; el estudio es una prueba de concepto
sources: [Häkkinen S, Voorhies WI, Willbrand EH, et al. Anchoring Functional Connectivity to Individual Sulcal Morphology Yields Insights in a Pediatric Study of Reasoning. J Neurosci 2025;45(26):e0726242025. doi:10.1523/JNEUROSCI.0726-24.2025]
---
## Pregunta del usuario (2026-10-02)

¿Este estudio sirve para entrenar un LLM o para algo de mmorch?

## Qué mide el estudio

- 43 chicos de 7 a 18 años, fMRI durante una tarea de razonamiento, unos 12.7 minutos útiles por participante.
- 42 surcos por persona, etiquetados a mano en la corteza prefrontal lateral y la parietal lateral.
- Los surcos se distinguen por su patrón de conectividad (96% en clasificación por pares) mejor que parches de corteza del mismo tamaño y forma.
- Los surcos se agrupan por conectividad y no por cercanía.
- En la mitad de los surcos chicos que probaron, un surco más profundo tiene más centralidad en la red.

## Veredicto: no aplica

- El dominio no se transfiere: un LLM no tiene surcos ni desarrollo; "profundidad → centralidad" queda como metáfora.
- El estudio no relaciona la conectividad con el puntaje de razonamiento de cada participante; el vínculo con el razonamiento viene de un estudio anterior (Voorhies 2021).
- La muestra es chica (n = 43) y no tiene réplica.
- Los autores admiten que el fMRI por participante no alcanza para una conexión individual confiable.
- Los autores admiten un confusor: un surco grande promedia más vértices, tiene más señal y puede parecer más central.
- Hay muchas comparaciones y análisis exploratorios; el resultado es correlacional.
- mmorch no entrena modelos: rutea y verifica con oráculos, y el estudio no trae tarea, dataset ni oráculo utilizable.

## Lo transferible

Una práctica de método, general y no exclusiva de este estudio: comparar cada unidad contra un modelo nulo emparejado por tamaño. En los bancos de mmorch, un detector (por ejemplo el informe de impacto) se mediría también contra un conjunto al azar con la misma cantidad de archivos.
