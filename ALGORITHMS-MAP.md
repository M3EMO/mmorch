# Mapping: 40 algoritmos → rol en mmorch (director / fábrica / utilitario)

La metáfora: **mmorch = director** (orquestación determinista). Los algoritmos se parten en
TRES toolboxes. Status: ✅ ya en mmorch · ⏳ pull cuando un problema MEDIDO lo pida · ❌ skip
(fuera de scope single-user/localhost). **Regla invariante: un algoritmo entra cuando un
problema medido lo pide, no porque exista (anti scope-creep).**

## 🎼 DIRECTOR — orquestación (CÓMO DECIDE qué nodo usar) → core
| # | Algoritmo | Rol | Status |
|---|---|---|---|
| 9 | Thompson sampling | el bandit (elige brazo) | ✅ feedback.ThompsonBandit |
| 10 | UCB | bandit con garantías (alt/complemento) | ⏳ |
| 21 | Q-Learning / DQN | RL de políticas | ❌ overkill (bandit alcanza) |
| 22/23 | MCTS / UCT | planificación de evolución en árbol | ⏳ (Fase 4 profunda) |
| 17 | A* | pipeline de acciones óptimo (si hay grafo) | ⏳ |
| 20 | Dijkstra | caminos mínimos / deps | ⏳ |
| 39 | Topo-sort (Kahn) | orden de deps de fases/tareas | ✅-ish (trivial, ya implícito) |
| 40 | Set-cover greedy | selección mínima de verificadores | ⏳ |
| 5/29 | Bayesian-opt / Hyperband | tuning de hiperparámetros de la NN | ⏳ (Fase 5/7) |
| 14/15/16/18/33 | PSO / GA / SA / ABC / Tabú | metaheurísticas de búsqueda | ⏳/❌ (GA→code-evo ya lo cubre innovate+tournament) |

## 🏭 FÁBRICA — construir modelos (CÓMO se ARMAN los solistas) → factory, en WSL+torch
| # | Algoritmo | Qué solista CONSTRUYE | Status |
|---|---|---|---|
| 11 | Regresión logística | clasificador binario (el toy actual) | ✅ factory.train_logreg |
| 1 | DNN / MLP (UAT) | router cerebral Capa A (v1.0) | ⏳ Fase 7 |
| 2 | GBRT | predictor de coste/latencia tabular | ⏳ (mejora de predict.py) |
| 3 | Random Forest | code-quality tabular (robusto a ruido) | ⏳ |
| 4 | SVM | clasificador de seguridad de cambios | ⏳ |
| 6/30 | Gaussian Process / Bayesian Ridge | predicción CON incertidumbre | ⏳ (calibración avanzada) |
| 25 | VAE | embeddings generativos de prompts | ⏳ |
| 26 | Contrastive / SimCLR | **code-embedder** (el que GANA a bge-small) | ⏳ **prioritario** |
| 27 | Self-Attention / Transformer | modelo de secuencia sobre código | ⏳ |
| 28 | Mixture of Experts | ensemble/escalado condicional | ⏳ |
| 8/34 | HMM / Viterbi | secuencias de tareas | ⏳ niche |
| 24 | Expectation-Maximization | clustering latente de episodios | ⏳ niche |

## 🔧 UTILITARIO — soporte
| # | Algoritmo | Uso | Status |
|---|---|---|---|
| 13 | k-NN | recall por similitud / shadow prior | ✅ (recall) + ⏳ Fase 5 |
| 19 | Tabla hash | caché de decisiones | ✅ memo (cache.py) |
| 12 | PCA | reducción de dim de embeddings | ⏳ |
| 7 | Filtro de Kalman | tracking de drift de reward | ⏳ (backstop) |
| 31 | Isolation Forest | anomalías en logs / regresión gradual | ⏳ (backstop) |
| 32 | DBSCAN | clustering de notas | ⏳ |
| 36 | Bloom filter | dedup rápido en loop_until_done | ⏳ |
| 35 | FFT | periodicidades en series temporales | ⏳ |
| 37 | Raft (consenso) | estado distribuido | ❌ single-user, NO |
| 38 | Set reconciliation | sync de memorias distribuidas | ❌ single-user, NO |

## Conclusión
- **Sí, los algoritmos sirven pa ARMAR modelos** — la sección 🏭 FÁBRICA es exactamente eso
  (cómo construir los solistas). Hoy la fábrica solo tiene logreg; pa solistas reales usaría
  GBRT/RF (tabular) y **SimCLR/Transformer** (code-embedders, lo prioritario tras medir que
  bge-small falla).
- **Director ≠ Fábrica**: el director DECIDE (bandit/UCB/tournament, core determinista); la
  fábrica CONSTRUYE (en WSL+torch, como nodos gateados). mmorch sigue siendo el conductor.
