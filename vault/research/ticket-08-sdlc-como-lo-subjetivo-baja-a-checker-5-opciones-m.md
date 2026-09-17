---
title: Ticket 08 SDLC: como lo subjetivo baja a checker — 5 opciones medidas por otros (2026-09)
created: 2026-09-15
tags: [research, mmorch, sdlc, gates, llm-judge, rubric, tdd, ticket-08]
status: seed
confidence: 0.7 (lectura de abstracts/resumenes; numeros citados, no replicados)
sources: [https://arxiv.org/html/2608.22559, https://arxiv.org/pdf/2606.26300, https://www.langchain.com/resources/llm-as-a-judge, https://arxiv.org/pdf/2605.19665, https://arxiv.org/html/2608.16742, https://arxiv.org/html/2604.26615v1, https://galileo.ai/blog/calibrate-llm-judge-human-annotations]
---
Pregunta del ticket 08 (mapa sdlc-6-gates): un gate de juicio (¿el test de aceptacion / la spec es el producto correcto?)
no tiene oraculo. ¿Como produce ejemplos etiquetados y baja a checker sintetizado? Web search 2026-09-15.

## Opciones que otros usan

A. **Rubrica -> funcion Python ejecutable** (ExecRubrics, 2608.22559). Un LLM traduce la rubrica en lenguaje natural a
   funciones de scoring (con herramientas NLP deterministas). Preference accuracy vs rubrica en texto: ArgQuality .91 vs .76,
   HelpSteer .70-.75 vs .62, HealthBench .42-.53 vs .52 (peor). Limite declarado: gameable ("disparar el check textual sin
   cumplir la intencion") y no evaluan interpretabilidad humana. = nuestro `synth_store` aplicado a la rubrica de la spec.
B. **Juez calibrado con correcciones humanas** (LangSmith Align Evals; Galileo). Loop: el juez puntua, humanos corrigen
   una muestra, las correcciones se vuelven few-shot del juez, se mide acuerdo (kappa) y se itera. Kappa tipico 0.3-0.4 al
   arrancar -> 0.6-0.75 tras 3-4 vueltas; mueven mas los ejemplos BORDE que los obvios; una rubrica por intencion.
C. **Criterios extraidos de las explicaciones humanas** (CriterAlign, 2605.19665, code preference). De los motivos que da
   el humano se extraen criterios recurrentes; el juez evalua criterio por criterio (binarios) antes del veredicto global.
   Pocos pares etiquetados alcanzan. Puente natural entre B y A: cada criterio binario es candidato a check determinista.
D. **Calidad del test por oraculo, no por juicio** (TDD-Agent, 2608.16742; TDD Governance, 2604.26615). El test se valida
   con ejecucion: rojo antes del codigo, cobertura y MUTATION SCORE (MutPy, 20 mutantes/problema); las tres metricas suben
   con las iteraciones. Sin aprobacion humana. RepoEval pass@1: 61->78 (GPT), 84->91 (DeepSeek) vs mini-SWE-agent.
   TDD Governance: rojo-primero + reparacion acotada (3) + rollback; sin numeros.
E. **Hibrido explicito** (Verification Horizon, 2606.26300): tests, jueces LLM, rubricas y humanos fallan en lugares
   distintos (tests no capturan intencion; jueces son inconsistentes y hackeables; rubricas no generalizan; humanos caros).
   Recomiendan: checks ejecutables como piso + etiquetas humanas SOLO en los casos ambiguos + varias capas.

## Lectura para mmorch (refutar por default)

- Nuestro plan D1/D2 (veredicto = test + etiqueta + motivo; 3 buenos + 3 malos -> synth) es la opcion A alimentada por C.
  Lo que faltaba: (1) el MOTIVO es la materia prima de los criterios (C), no un campo decorativo; (2) el checker sintetizado
  entra solo con acuerdo medido contra el humano (B: kappa, no "3+1"); (3) es gameable (A): el gate ejecutable convive con
  el humano en los casos borde (E), no lo reemplaza.
- D mueve parte del juicio a un oraculo que YA tenemos en el ticket 13: mutation score del test de aceptacion sobre el
  codigo final. "¿El test mide la tarea?" deja de ser opinion en la mitad de los casos: rojo en HEAD + mata mutantes.
- Numero de referencia para el umbral de promocion: kappa >= 0.6 con el humano sobre los casos borde (B); antes de eso el
  checker solo OBSERVA (shadow), como el rollout del auto-apply.
