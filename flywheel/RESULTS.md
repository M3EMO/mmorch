# Flywheel — resultados (2026-06-10)

Objetivo: entrenar `model:code_embedder` que ENTIENDA código (signal = ejecución/estructura,
no imitación LLM) y ver si bate a bge-small. Cuello previo: "code-quality desde texto = azar".

## Setup
- Encoder SimCLR: token-embed → bi-GRU → mean-pool (dim 256), NT-Xent, aug = rename
  consistente de identificadores + token-dropout. `flywheel/simclr.py`. WSL+torch CPU.
- Baselines: STRUCT (9 feats AST, `factory.featurize_code`), BGE-small frozen (384d).
- Eval: linear probe (logreg), 5-fold AUC. Para oracle: GroupKFold por spec (cross-spec).

## Resultados

| Label (señal) | n | within-spec AUC | cross-spec AUC | ganador |
|---|---|---|---|---|
| **JIT-defect** (buggy/fixed, función aislada) | 2k | — | **0.47–0.51 = AZAR** | ninguno |
| **radon MI** (mantenibilidad, estructura) | 6k | — | STRUCT 0.84 · bge 0.80 · **SimCLR 0.88** | SimCLR |
| **ejecución** (pass/fail, adversarial) | 480 | bge 0.68 | STRUCT 0.50 · bge 0.535±0.15 = azar | (ninguno cross) |

## Conclusiones (con datos)
1. **El cuello era la LABEL, no la representación.** JIT-defect aislado no tiene señal en el
   texto (el bug es semántico, necesita diff/tests). Cuando la señal SÍ está (radon), el
   harness aprende.
2. **SimCLR bate a bge-small Y a struct** en estructura/calidad (0.88 vs 0.80/0.84). El
   encoder contrastivo entrenado solo con tokens+aug es mejor representación de código que
   bge-small frozen. → `model:code_embedder` es un nodo real, entrenable, promovible.
3. **Correctitud NO se aprende estático.** Embedding detecta buggy-vs-correct dentro de specs
   conocidos (0.68) pero NO generaliza a specs nuevos (0.535 = azar). Detectar correctitud de
   código arbitrario **requiere EJECUCIÓN** — exactamente lo que los checkers deterministas
   ya dan (costo 0, cross-family innecesario).

## Arquitectura resultante del flywheel
- **Estructura/calidad** → encoder SimCLR (`code_embedder.pt`). Reemplaza a bge en ese rol.
- **Correctitud** → checkers de ejecución (`checkers.python_exec/unit_test`). Oráculo, no modelo.
- El "generador/entendedor no-LLM" (seed) se parte igual: parte estructural = aprendible
  (hecho), parte semántica = ejecutar, no embeber.

## Repro
- `flywheel/relabel_radon.py` → `logs/radon_dataset.jsonl` (shuffle obligatorio: sin él un
  prefijo es mono-clase).
- `flywheel/oracle_dataset.py [K]` → genera (DeepSeek, mitad adversarial) + etiqueta por
  `python_exec` (NO `unit_test`: asserts a nivel módulo = "no tests ran" en pytest).
- WSL: `MSYS2_ARG_CONV_EXCL='*' wsl ... 'export MMORCH_DS=/mnt/c/...; ~/flywheel/bin/python ...'`
  (MSYS mangla paths `/mnt/c`).
- Baselines: `MMORCH_DS_WIN=<win path> .venv/Scripts/python.exe flywheel/baseline.py [N]`.

## Pendiente / next
- Escalar SimCLR estructural: más epochs + dataset 10k completo + GBRT probe → ¿0.90+?
- code_embedder como nodo: wire en `nodes.py` (status active) + usarlo en code_quality checker
  en vez de bge. Gate por evolve (zona azul: reversible).
- Correctitud a escala real: más specs (>100) para cross-spec, o aceptar que es trabajo de
  ejecución y no de embedding.
