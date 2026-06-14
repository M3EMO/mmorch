# Weights de mmorch — funcionalidad, data-flow, construcción eficiente

Investigación (2026-06-14) sobre cómo armar los pesos de nodos neuronales eficientemente,
qué hacen, y la lógica de flujo de datos. Único peso hoy: `code_embedder` (SimCLR).

## 1. Qué HACEN los pesos (funcionalidad)

`code_embedder` mapea **código → vector 256d** donde código funcional/estructuralmente
parecido queda cerca. No es "entender sintaxis" — es un ESPACIO de similitud de código
aterrizado en estructura (probado: bate a bge-small en radon-tier, 0.88 vs 0.80).

Consumidores (a quién le sirve el embedding):
- **`shadow_prior`** (Fase 5): embeddings del CONTEXTO → prior k-NN sobre el bandit. Mejor
  espacio = mejor ruteo. (Medido: code_embedder +0.168 vs bge +0.128 en outcomes de código.)
- **`checkers.code_quality`** / clasificadores de la fábrica: feature de calidad.
- **`memory.recall`**: podría embeber notas de CÓDIGO mejor que bge (hoy usa bge).

El peso es un **órgano de percepción** del orquestador: convierte código en algo sobre lo
que la lógica determinista puede rutear/rankear/recordar.

## 2. Data-flow (la lógica del flywheel)

```
TRABAJO REAL (rubric_loop/code_loop/project_loop)
   └─> trajectory.py: (código, label=EJECUCIÓN)   [+ datasets: radon, oracle]
          └─> TRAIN (WSL+torch): SimCLR contrastivo sobre pares aug   [flywheel/simclr.py]
                 └─> EXPORT torch→numpy .npz   [flywheel/export_numpy.py]
                        └─> weights/manifest.json (sha256 + métrica + regen)
                               └─> INFER numpy puro (core, sin torch)   [mmorch/code_embedder.py]
                                      └─> CONSUMEN: shadow_prior · code_quality · recall
                                             └─> GATE evolve: vNueva debe BATIR la métrica
                                                    └─> (loop) más trabajo → más data → retrain
```

Clave (AlphaZero-style, NO distilación de LLM): la **label es EJECUCIÓN** (tests/checkers),
no la opinión del LLM. Por eso el encoder PUEDE superar al maestro — aprende de la verdad de
campo, no imita. El LLM es generador de datos; el oráculo es la ejecución.

## 3. Cómo construirlos EFICIENTE (hallazgos + plan)

### a) Cola de negativos estilo MoCo (el lever #1 pa CPU)
SimCLR puro necesita batches enormes (4096–8192) pa tener negativos → caro. **MoCo** desacopla:
batch chico (256) + **cola de 65k negativos** de batches previos. → muchos más negativos sin
batch gigante = ideal pa WSL/CPU. ([SimCLR vs MoCo](https://milvus.io/ai-quick-reference/what-are-the-differences-between-simclr-and-moco-two-popular-contrastive-learning-frameworks))
→ Acción: agregar memory-bank queue a `simclr.py`.

### b) Positivos por EQUIVALENCIA FUNCIONAL (mejor que aug sintáctica)
[ContraCode](https://github.com/parasj/contracode) aprende FUNCIONALIDAD, no forma: positivos =
variantes que hacen lo mismo. Hoy nuestros positivos = rename+dropout (sintáctico, débil).
**Tenemos algo mejor gratis**: en `oracle_dataset`, dos soluciones que PASAN el mismo spec son
**funcionalmente equivalentes** = positivos REALES (no aug). Señal mucho más fuerte.
→ Acción: positivos = soluciones-que-pasan-el-mismo-spec; negativos = otras specs / soluciones que fallan.

### c) Augmentación AST/compiler (si falta data)
ContraCode usa un compilador source-to-source pa generar variantes; [TransformCode](https://arxiv.org/html/2311.08157v2)
usa transformaciones de subárbol AST. Más fuerte que rename+dropout: reordenar statements
independientes, dead-code, constructos equivalentes. → mejora `augment()` en simclr.py.

### d) Data-flow graph (GraphCodeBERT) — seed mayor
[GraphCodeBERT](https://arxiv.org/abs/2007.04973) consume el GRAFO de control/datos, no tokens
→ más cerca de "entender el flujo". Es el nodo `gen:synth`/GNN-AST ya parkeado. Salto grande, costoso.

### e) Distilación pa arrancar (convergencia rápida en CPU)
[LEAF](https://www.mongodb.com/company/blog/engineering/leaf-distillation-state-of-the-art-text-embedding-models)
destila embedders SOTA a 23M params CPU-friendly. Init desde bge/un teacher → converge más
rápido que from-scratch. Opción: inicializar el embedding-table desde bge y fine-tune contrastivo.

### f) Higiene (ya parcial)
- projection head se DESCARTA en inferencia (usamos mean-pool, ok).
- temperature schedule (subir gradual) reduce penalización de falsos-negativos.
- **fp16/int8** pa deploy (seed) — hoy 3.77MB float32, innecesario.

## 4. Recomendación priorizada
1. **(b) Positivos funcionales del oracle** — señal real, cero costo extra, el mayor salto de calidad.
2. **(a) Cola MoCo** — el lever de EFICIENCIA pa CPU (más negativos, batch chico).
3. **(c) Augmentación AST** — si la data escasea.
4. **(e) Distilación-init** — convergencia más rápida.
5. **(d) GraphCodeBERT/GNN** y **(f) cuantización** = seeds mayores.

Todo gated: una versión nueva del peso solo se promueve si BATE la métrica del incumbente
(`weights/manifest.json`). El peso es cache regenerable (`regen_cmd`), no fuente.
