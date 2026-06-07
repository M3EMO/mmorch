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

---

# RUN SERIO (n=54, same-tier) — 2026-06-07

Corregidos 2 de 3 confounds: n=54 (vs 8) + same-tier (gemini-flash CROSS vs
deepseek-chat SAME, AMBOS non-thinking — sin confound thinking/flash). Benchmark
PROGRAMATICO con ground-truth deterministico (aritmetica/porcentaje/paridad/comparacion
/silogismos), 25 correctos + 29 con error plantado. Script: `ablation_run.py`.

| verifier | fam | acc | false_pass | false_refute | cost | lat |
|---|---|---|---|---|---|---|
| gemini-2.5-flash | CROSS | 0.93 | 0/29 | 4 | $0.0135 | 6.2s |
| deepseek-chat | SAME | 0.87 | 0/29 | 7 | $0.0020 | 1.5s |

## Hallazgos
1. **AMBOS: 0 false_pass (0/29 bugs dejados pasar).** El error peligroso del verificador
   (dejar pasar un bug) fue CERO para los dos. Same-family barato no perdio deteccion.
2. La brecha de accuracy (0.93 vs 0.87) es TODA false_refute (sobre-escepticismo en
   correctos): cross 4, same 7. Cross mejor en PRECISION, no en deteccion.
3. n=54, diff de 3 false_refutes / 25 correctos. Borderline, no significativo fuerte.
4. El benchmark es objetivamente checkeable -> SIN punto ciego family-specific. Por eso
   ambos aciertan los bugs. La tesis cross-family (decorrelacion de SESGOS compartidos)
   sigue sin testearse: requiere artifacts subjetivos/dificiles donde el error correlacione
   por familia.

## Conclusion (revisada)
La tesis cross-family NO quedo validada (ambos cazan 100% de bugs en tareas checkeables).
Emergio un matiz: para verificacion OBJETIVAMENTE CHECKEABLE (mates/codigo/hechos),
same-family barato rinde IGUAL en deteccion a 1/6.75 del costo y 4x mas rapido. Cross
pago 6.75x por menos false-refutes, no por cazar mas bugs.

## Propuesta (NO cambio — disciplina [PROPOSAL])
Escopar el invariante "cross-family obligatorio":
- **Mandatorio** para artifacts SUBJETIVOS / alto-riesgo donde el punto ciego correlaciona
  por familia (diseno, juicio, sintesis, claims no-checkeables).
- **Opcional** (same-family barato basta) para verificacion OBJETIVAMENTE CHECKEABLE
  (aritmetica, codigo, hechos verificables) — 0 perdida de deteccion, 6.75x mas barato.
Requiere antes: test con artifacts ADVERSARIOS family-specific + ensemble-vs-ensemble.
Hasta entonces, el invariante se mantiene como esta.

---

# RUN ADVERSARIO (n=30 HARD, errores sutiles) — 2026-06-07

Benchmark HARD hand-authored: 16 errores SUTILES (escalado vol √t, base-rate,
conjuncion, mutable-default, float ==, affirming-consequent, Chimborazo/Everest, CAGR,
real-return exacto) + 14 correctos. 3 verificadores x 30 casos. Script:
`ablation_adversarial.py`. Costo $0.0107.

|  | acc | false_pass |
|---|---|---|
| deepseek-A (SAME) | 0.97 | 0/16 |
| deepseek-B (SAME', temp .4, prompt variante) | 0.93 | 0/16 |
| gemini (CROSS) | 0.97 | 0/16 |

## Hipotesis -> veredicto
- **H1** (deepseek tiene blind-spots, false_pass>0): **REFUTADA**. Cazo los 16. Cero
  punto ciego a esta dificultad.
- **H2** (gemini caza los blind-spots de deepseek): **N/A** — no hay blind-spots.
  Jaccard=1.00 son sets vacios (artefacto), sin significado.
- **H3** (ensemble cross-2 < same-2 en false_pass): **sin diferencia** (ambos 0).

Las unicas misses fueron false_REFUTE (rechazar correctos), no fallo de deteccion.

## Conclusion (definitiva para benchmarks checkeables)
La tesis cross-family NO quedo soportada por NINGUN benchmark (objetivo n=54 + hard
n=30). Para verificacion de claims CHECKEABLES — incluso sutiles, multi-dominio — un
solo verificador same-family BARATO (deepseek) caza 100% de errores a ~6x menos costo
que cross-family. Esfuerzo razonable de trampas adversarias no logro surfacear ni un
blind-spot. Resultado NEGATIVO fuerte.

## Limite de validez (donde cross-family AUN podria valer — NO testeado)
1. **Artifacts SUBJETIVOS sin ground-truth** (diseno, juicio, sintesis, gusto): ahi
   "blind spot" = sesgo compartido no-detectable como error. Mis benchmarks tienen
   verdad objetiva -> no pueden testearlo.
2. **AUTO-verificacion**: use artifacts hand-authored, NO generados por deepseek. La
   version fuerte de la tesis (un modelo no ve SU PROPIO error) queda sin testear. Ese
   es el experimento decisivo restante: deepseek GENERA -> deepseek-verify vs gemini-verify.
3. Solo 2 familias disponibles -> sin test de 3-familia.

## [PROPOSAL: STRATEGY CHANGE] (candidato, NO aplicado)
Escopar el invariante "cross-family obligatorio" del diseño §4:
- **Mandatorio** SOLO para artifacts subjetivos / sin ground-truth / alto-riesgo
  (diseno, juicio estrategico, sintesis abierta, claims no-checkeables) Y para
  AUTO-verificacion (modelo revisando su propio output).
- **Opcional** (same-family barato basta, 0 perdida de deteccion, ~6x mas barato) para
  verificacion de claims OBJETIVAMENTE CHECKEABLES (mates, codigo, hechos, logica formal).
Gate para aplicar: correr el test de AUTO-verificacion (limite #2). Si ahi tampoco
aparece decorrelacion -> escopar. Si aparece -> el invariante se sostiene para
self-verify. Disciplina: NO cambiar el invariante sin ese run.
