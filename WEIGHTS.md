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

## 3b. Resultados de la ablación (2026-06-14)
- **Encoder es ESTRUCTURAL, no funcional**: en data diversa (oracle_diverse, 367 passers/20 specs)
  el code_embedder se desploma (P@1 0.99→0.45) — agrupa por superficie, no por función.
- **#2 MoCo: RECHAZADO** (radon 0.884 < 0.899 NT-Xent; dataset chico → in-batch alcanza).
- **retrain full-config: ADOPTADO** (0.88→0.899, dim 256→384) + **fp16 (#5): ADOPTADO** (½ tamaño, lossless).
- **#1 positivos funcionales**: A/B vs rename-aug, P@1 held-out, name-normalized, n=5 seeds →
  lift medio **+0.024 (std 0.031, 4/5 positivos, 1 negativo)**. DIRECCIONAL pero NO significativo
  a 20 specs/4 held-out. Mecanismo y pipeline construidos (`simclr_functional.py`, `oracle_diverse.py`).
  **NO promovido** (varianza > efecto). Cuello = spec-count: confirmar con 40-100 specs.

## 3c. Embedding por EJECUCIÓN (exec-embedding) — VALIDADO (2026-06-15)
El fix funcional REAL del seed, no una red más grande: `mmorch/exec_embedder.py`. **CERO entrenamiento.**
Corre la función en sondas deterministas por arity (sandbox) → canonicaliza outputs (dict/set sin
orden → mismo canon) → feature-hash de (sonda→output) a vector 256d. Funcional-equiv → mismo vector.

Medición sobre **oracle_diverse**, apples-to-apples (`eval_functional.py code exec hybrid`):

| arm | 20 specs (367 passers) | 40 specs (221 passers) |
|-----|------------------------|------------------------|
| structural (dim 384) | 0.847 P@1 / 0.710 AUC | **0.430** P@1 / 0.665 AUC |
| **behavioral (exec)** | **0.973** / 0.948 | **0.919** / **0.948** |
| hybrid (struct⊕behav) | 0.970 / 0.925 | 0.860 / 0.917 |

- **El scale-up 20→40 specs (C) reveló lo clave**: el structural se DESPLOMA (0.847→0.430) cuando
  hay más specs y menos densidad/spec — su "competencia" a 20 specs era artefacto de spec-count
  bajo/saturado (justo el colapso ~0.45 que §3b vio con el encoder viejo). **behavioral AGUANTA**
  (0.92, AUC 0.95). El gap se abre de **+0.13** (20 specs) a **+0.49 P@1** (40 specs).
- **behavioral GENERALIZA, structural se DEGRADA con spec-count.** A escala, behavioral-SOLO es el
  ganador claro; **hybrid empeora** (0.86) porque el structural débil (0.43) lo arrastra. Hybrid solo
  convenía cuando el structural era decente (20 specs).
- Cobertura de sondas (A, 2026-06-15): canon float-tolerante + timeout POR-SONDA + captura de
  mutación in-place + detección por `callable` + sin sonda degenerada n=0 → **drop 0/221** (antes
  8/367). Sigue acotado a código ejecutable nivel-función (arity 1/2/3).
- exec-embedding NO reemplaza al code_embedder en su EJE (radon/estructura). Es el encoder FUNCIONAL:
  para similitud de comportamiento, behavioral-solo > structural > hybrid a escala.
- **Stop point**: validado + documentado. Adapter `embed_hybrid` pluggable construido (B) pero los
  outcomes de shadow_prior son ETIQUETAS, no código → exec N/A ahí hoy; NO cableado a consumidores
  live (cambio gated aparte). Detalle en docstring de `shadow_prior.py`.

## 4. Recomendación priorizada
1. **(b) Positivos funcionales del oracle** — señal real, cero costo extra, el mayor salto de calidad.
2. **(a) Cola MoCo** — el lever de EFICIENCIA pa CPU (más negativos, batch chico).
3. **(c) Augmentación AST** — si la data escasea.
4. **(e) Distilación-init** — convergencia más rápida.
5. **(d) GraphCodeBERT/GNN** y **(f) cuantización** = seeds mayores.

Todo gated: una versión nueva del peso solo se promueve si BATE la métrica del incumbente
(`weights/manifest.json`). El peso es cache regenerable (`regen_cmd`), no fuente.
