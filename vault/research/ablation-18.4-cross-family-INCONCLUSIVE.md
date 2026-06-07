---
title: "Ablacion §18.4 — cross-family verifier (PRIMER RUN, inconcluso)"
status: inconclusive
date: 2026-06-07
tags: [ablation, cross-family, verification, methodology]
---

# Ablacion §18.4 — cross vs same family verifier

## Setup
Mini-benchmark 8 casos ETIQUETADOS (truth_passed), mix correcto/error-plantado:
geometria, geografia, logica, codigo. Mismo verificador-esceptico (system
anti-sicofancia + parser), distinto modelo. author_model = deepseek-chat.

## Resultado (n=8)
| verifier | family | accuracy | false_pass | false_refute | cost | lat |
|---|---|---|---|---|---|---|
| deepseek-reasoner | SAME | 1.00 | 0 | 0 | $0.0009 | 4.1s |
| gemini-2.5-flash | CROSS | 0.88 | 0 | 1 | $0.0021 | 4.9s |

gemini false-refuto 'geom-ok' (area circulo r=3 = 28.27, correcta). Ambos: 0 false-pass.

## Veredicto: INCONCLUSO. No valida ni refuta la regla cross-family.

Confounds (por que este run no decide):
1. **n=8 ridiculo.** 1 miss = 0.12 swing. Cero significancia.
2. **Confound de capacidad.** deepseek-reasoner = THINKING; gemini-flash = non-thinking.
   La brecha de tier domina el efecto de familia. No es comparacion limpia.
3. **La tesis es de ENSEMBLE, no single-verifier.** Cross-family decorrelaciona errores
   en un jurado (caza el punto ciego que la familia del AUTOR comparte). Accuracy de UN
   verificador generico no testea decorrelacion.
4. **El miss de gemini fue false-REFUTE, no false-pass.** El error peligroso de un
   verificador es false-pass (deja pasar un bug). Ambos: 0 false-pass — los 4 bugs
   plantados cazados por los dos.

## Que haria falta para un test valido
- n >= 50-100 casos, balanceado.
- Modelos del MISMO tier (controlar capacidad): comparar gemini-flash vs un
  deepseek-chat no-thinking, no contra reasoner.
- Comparar ENSEMBLE same-family vs ENSEMBLE cross-family (la tesis real).
- Artifacts que ejerciten el punto ciego especifico de la familia del autor
  (donde same-family deberia FALLAR por compartir el sesgo).
- Repetir (varianza de muestreo a temp 0 es baja pero el set chico no).

## Implicancia operativa (ahora)
La regla cross-family se MANTIENE como invariante de diseno (decorrelacion es teoria
solida), pero queda marcada como NO validada empiricamente en este harness. El harness
`mmorch/ablation.py` queda listo para correr el test serio cuando haya benchmark grande.
No escalar conclusiones sin metricas verdes (diseño §14).
