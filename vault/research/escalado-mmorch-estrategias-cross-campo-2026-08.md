# Research: estrategias de otros campos para escalar el flywheel de auto-mejora de mmorch

Fecha: 2026-08-18. Contexto: mmorch ya tiene bandits Thompson+decay, verificación cross-family, mutation-testing+hardening, loop nocturno de ideas (144 pares nota×proyecto brute-force), auto-reflexión, feedback.jsonl con rewards. Meta futura: LoRA de modelos chicos, reward models, router aprendido.

---

## 1. Active learning + uncertainty sampling (label-efficiency clásico)

**Qué es.** En vez de etiquetar/juzgar todo, elegir los ítems donde el modelo está más inseguro o donde un comité de modelos disiente (query-by-committee). Décadas de evidencia: reduce el costo de etiquetado 2-10x manteniendo accuracy.

**Fuentes.**
- Survey LLM-based active learning: https://arxiv.org/html/2502.11767v1
- Active label acquisition para RLVR: https://arxiv.org/pdf/2605.25864
- Práctico: https://labelyourdata.com/articles/active-learning-machine-learning

**Mapeo a mmorch.** El loop nocturno juzga 144 pares nota×proyecto brute-force: reemplazar por (a) filtro barato de embedding-similarity que descarta pares obviamente irrelevantes (sim < umbral → skip, ~cero costo), (b) juzgar con LLM solo la franja incierta, (c) query-by-committee gratis: mmorch YA tiene cross-family — donde DeepSeek y Gemini disienten está la señal máxima; ese disenso debería *priorizar* qué se re-juzga y qué se guarda como dato de entrenamiento. Módulo: adjudicación nocturna + `feedback.jsonl` (loguear el score de incertidumbre por juicio).

**Esfuerzo.** Medio (el filtro por embeddings es trivial si ya hay recall con embeddings; el ranking por disenso es un campo extra en el log).

## 2. Curriculum automático por learning progress (Oudeyer / Graves)

**Qué es.** Elegir tareas no por dificultad absoluta sino por *derivada de mejora* (learning progress): practicar lo que está mejorando, abandonar lo dominado y lo imposible. Se formula exactamente como multi-armed bandit donde el reward del arm es el LP.

**Fuentes.**
- Graves et al., Automated Curriculum Learning: https://proceedings.mlr.press/v70/graves17a/graves17a.pdf
- Survey ACL for Deep RL (Portelas, Oudeyer): https://ar5iv.labs.arxiv.org/html/2003.04664
- Teacher-Student CL: https://www.researchgate.net/publication/318122053_Teacher-Student_Curriculum_Learning

**Mapeo a mmorch.** mmorch ya tiene bandits Thompson con decay — pero recompensan *éxito*, no *progreso*. Graft chico: un segundo reward = |Δ tasa de éxito reciente| por (modelo, tipo de tarea). Aplica a: (a) qué lentes del loop de ideas corren más seguido (las que producen candidatas que maduran → más presupuesto), (b) hillclimb/autoresearch: asignar trials donde la métrica todavía se mueve. Ataca directamente el problema del bandit "starved": LP concentra presupuesto donde hay señal. Módulo: bandit core + evolve_nightly.

**Esfuerzo.** Medio. Reusa la infraestructura de bandits existente.

## 3. Quality-Diversity / MAP-Elites (computación evolutiva)

**Qué es.** En vez de guardar la mejor solución, mantener un archivo grillado por dimensiones de comportamiento, con el elite de cada celda. Evita convergencia prematura y produce stepping stones: soluciones mediocres en una celda engendran las mejores de otra.

**Fuentes.**
- Diverse Prompts: MAP-Elites sobre espacio de prompts LLM: https://arxiv.org/abs/2504.14367
- Overview MAP-Elites: https://www.emergentmind.com/topics/map-elites-algorithm
- RainbowPlus (QD para generación adversarial de prompts): https://arxiv.org/pdf/2504.15047

**Mapeo a mmorch.** Dos aplicaciones concretas: (a) **candidatas del loop de ideas**: hoy la maduración tiende a un ranking lineal; grillar por (proyecto destino × tipo de idea: graft/refactor/research/tooling) y retener la mejor por celda evita que un solo proyecto/tipo monopolice; (b) **prompts/playbooks**: archivo de variantes de prompt por (largo × estilo × modelo objetivo), con fitness = reward medido de feedback.jsonl. NO adoptar un framework QD (pyribs etc.): a escala personal el "archivo" es un dict JSON con 10-30 celdas y una regla de reemplazo. Módulo: evolve_nightly / session_playbooks.

**Esfuerzo.** Trivial-medio (la versión dict; la versión completa con descriptores aprendidos NO aplica a esta escala — descartada).

## 4. SPC / control charts (control de procesos industrial)

**Qué es.** CUSUM y EWMA detectan shifts chicos y drifts sostenidos en una métrica de proceso, distinguiendo variación común de causa especial. Es la herramienta estándar de 70 años para "¿esto cambió de verdad o es ruido?" — exactamente el problema de "actividad que parece mejora".

**Fuentes.**
- CUSUM/EWMA charts (JMP): https://www.jmp.com/en/statistics-knowledge-portal/quality-and-reliability-methods/control-charts/cusum-and-ewma-control-charts
- Drift detection vía SPC en ML: https://arxiv.org/pdf/1704.00023
- Adaptive CUSUM para drift: https://users.phhp.ufl.edu/pqiu/research/YiQ22.pdf

**Mapeo a mmorch.** Correr EWMA (drift medio) + CUSUM (drift chico sostenido) sobre las series que ya existen: reward medio de feedback.jsonl, error-rate por modelo (el glm-4.6 34% err se habría alarmado solo), ECE, tasa de maduración de candidatas. La auto-reflexión pasa de narrativa LLM ("parece que mejora") a alarmas estadísticas ("CUSUM cruzó el límite: el reward medio cae hace 12 días"). Clave anti-pseudo-progreso: chartear métricas de *outcome* (reward, adopción de candidatas), nunca de *actividad* (notas escritas, juicios corridos). ~50 líneas de numpy, sin dependencias. Módulo: metrics_summary / evolve_self.

**Esfuerzo.** Trivial. El mejor ratio valor/esfuerzo de toda la lista.

## 5. Data engine / flywheel (patrón Tesla, escala personal)

**Qué es.** Loop collect → curar casos difíciles → etiquetar → entrenar → deploy → observar (shadow mode) → re-curar. Los 3 módulos del data engine: active learning (qué etiquetar), dataset cleaning (labels consistentes), y eval congelado que gatea cada deploy.

**Fuentes.**
- Data Engine Design: https://medium.com/@george.pearse/data-engine-design-9b29a20ff9f0
- NVIDIA data-flywheel blueprint (arquitectura concreta): https://github.com/NVIDIA-AI-Blueprints/data-flywheel/blob/main/docs/01-architecture.md
- Deepchecks, pitfalls: https://deepchecks.com/glossary/data-flywheel/

**Mapeo a mmorch.** Tres ideas robables a escala personal: (a) **eval set congelado y versionado** — sin un golden set fijo, el flywheel gira sin saber si avanza (mmorch ya lo aprendió con workflow-evolution: benchmarks frozen); formalizarlo como `evalsets/vN/` inmutable; (b) **shadow mode** para el futuro router aprendido: el router LoRA predice en paralelo al bandit real, se loguea su acierto SIN darle control, hasta que gane en shadow — patrón Tesla directo, elimina el riesgo de deploy; (c) **minar el long tail**: los fallos y disensos de feedback.jsonl son los "clips curados", no los éxitos rutinarios. DESCARTADO: toda la parte de infra (data lake, pipelines distribuidos, auto-labeling masivo) — a escala personal es JSONL + carpetas versionadas.

**Esfuerzo.** Medio (eval congelado trivial; shadow mode medio).

## 6. Formato de datos para entrenar después (RLVR / rejection sampling / DPO)

**Qué es.** RS-DPO y statistical rejection sampling muestran el pipeline estándar: generar k respuestas por prompt, rankear con verifier/reward, guardar pares (chosen, rejected). RLVR usa labels binarios de verificadores deterministas. El formato que se acumula HOY determina qué se puede entrenar en 2 años.

**Fuentes.**
- RS-DPO: https://aclanthology.org/2024.findings-naacl.108.pdf
- Statistical Rejection Sampling (RSO): https://openreview.net/forum?id=xbjSwwrQOe
- Guía práctica SFT/DPO/RFT: https://cookbook.openai.com/examples/fine_tuning_direct_preference_optimization_guide

**Mapeo a mmorch.** feedback.jsonl con rewards escalares NO alcanza para DPO. Acumular desde ya, en cada verificación cross-family y cada tournament/bucket_rank:
1. **Pares DPO**: `{prompt, chosen, rejected, judge, margin}` — cada refutación de Gemini a DeepSeek YA es un par; hoy se tira.
2. **Verifier labels** (para reward model / RLVR): `{input, output, verdict binario, razones}` de cada gate (ruff/mypy/tests = verificadores deterministas gratis, oro para RLVR).
3. **Trayectorias de ruteo** (para el router): `{features de la tarea, modelo elegido, outcome}` — probablemente ya casi existe en feedback.jsonl; asegurar que las features del momento de decisión queden guardadas, no solo el outcome.
Regla: guardar SIEMPRE el contexto completo del prompt (reproducible), el perdedor además del ganador, y el porqué del veredicto. Módulo: adversarial_verify / tournament / record_outcome — un hook de logging, no lógica nueva.

**Esfuerzo.** Trivial de instrumentar HOY; imposible de reconstruir retroactivamente. Urgencia máxima por eso.

## 7. Bonus otro campo: sistema inmune (clonal selection) y ecología

**Qué es.** CLONALG: clonar los anticuerpos de mayor afinidad, hipermutarlos inversamente proporcional a su afinidad (los buenos mutan poco, los mediocres mucho), matar los no estimulados, y mantener memoria de largo plazo separada del repertorio activo. A diferencia de un GA, converge a un *conjunto diverso* de óptimos locales.

**Fuentes.**
- de Castro & Von Zuben, CLONALG: https://scispace.com/pdf/learning-and-optimization-using-the-clonal-selection-fjnwsvdguv.pdf
- Wikipedia (resumen mecanismos): https://en.wikipedia.org/wiki/Clonal_selection_algorithm

**Mapeo a mmorch.** Dos préstamos: (a) **mutación inversa a la afinidad** para la maduración de candidatas: las ideas con buen track record se refinan con cambios chicos, las mediocres-pero-vivas reciben re-escrituras agresivas por la lente — regla de un if, mejora la explotación/exploración de la maduración; (b) **memoria inmune vs repertorio activo**: separar el pool caliente de candidatas (con decay agresivo) de una memoria chica de "patrones que funcionaron" que no decae y se re-inyecta cuando reaparece un problema similar — esto es literalmente la intuition-layer diseñada, la biología valida el diseño. DESCARTADO: implementar CLONALG completo como optimizador — sus nichos (pattern recognition binario) no aplican; robar los 2 mecanismos, no el algoritmo.

**Esfuerzo.** Trivial (a), ya-diseñado (b).

## Descartes explícitos (no aplican a escala personal)

- **Frameworks QD (pyribs) y descriptores aprendidos**: overkill; el archivo es un dict.
- **Infra de data engine** (lakes, auto-labeling distribuido, fleet telemetry): la escala es JSONL.
- **Expected error reduction / BALD y active learning bayesiano pesado**: requiere reentrenar el modelo por query; el proxy embedding+disenso da el 80%.
- **Control charts multivariados (Hotelling T²)**: pocas series y correlacionadas a mano alcanza; empezar univariado.
- **RLHF completo con humanos etiquetando**: el humano es 1; los labels vienen de verificadores y cross-family, no de anotación manual.
- **Entrenar el LoRA/reward model YA**: con 8GB RAM y pocos miles de ejemplos es prematuro; la jugada correcta 2026 es *acumular el formato correcto* (sección 6) y entrenar cuando llegue el hardware de 64GB.

## Top-5 por valor/esfuerzo

1. **Instrumentar formato de entrenamiento (§6)** — trivial hoy, irrecuperable después; convierte cada verificación en dato DPO/RLVR. Hacer esta semana.
2. **CUSUM/EWMA sobre feedback.jsonl (§4)** — trivial, ataca directamente el pseudo-progreso ya detectado por la reflexión; alarmas objetivas.
3. **Adjudicación por incertidumbre + disenso (§1)** — corta el brute-force 144→~30 juicios y los disensos alimentan el punto 1.
4. **Eval set congelado + shadow mode (§5)** — el gate que hace honesto todo lo demás; shadow mode desriesga el futuro router.
5. **LP-bandit + archivo MAP-Elites liviano para candidatas (§2+§3)** — mismo módulo (evolve_nightly), presupuesto donde hay progreso y diversidad garantizada por celda.
