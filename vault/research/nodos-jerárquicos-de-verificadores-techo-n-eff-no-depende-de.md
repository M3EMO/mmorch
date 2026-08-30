---
title: Nodos jerárquicos de verificadores — techo n_eff no depende de la arquitectura, depende del piso de correlación global
created: 2026-08-29
tags: [research, orchestration, research, decision-theory, ensemble, verification, evolve, llm-as-judge, follow-up]
status: applied
confidence: 0.85
sources: [https://cran.r-project.org/web/packages/PracTools/vignettes/Design-effects.html, https://arxiv.org/html/2605.29800]
---
## Follow-up de [[decision-systems-majority-voting-requiere-independencia-pane]]

Pregunta del usuario: ¿y si en vez de repetir el mismo modelo, armamos M NODOS
(cada uno = 3 familias de LLM con pesos/criterios distintos) y votamos entre nodos?
¿Cuál sería el n efectivo?

## Modelo (design-effect de dos niveles, cluster sampling)

Descomposición de varianza de cada veredicto: `σ² = σ²_G + σ²_N + σ²_e` —
G = sesgo GLOBAL compartido por todo LLM (datos/RLHF/normas de safety-tuning
compartidas), N = sesgo compartido dentro de un mismo nodo, e = ruido propio del
juez. Con M nodos de k jueces:

```
n_eff(M,k) = σ² / [σ²_G + σ²_N/M + σ²_e/(Mk)]
```

Cuando M,k → ∞: `n_eff → σ²/σ²_G = 1/ρ_global`. Agregar nodos mata el ruido de
nodo/juez pero NUNCA el piso global — la arquitectura jerárquica no cambia el techo
asintótico, solo qué tan rápido lo alcanzás.

## Techo empírico

Con φ̄=0.39 medido en "Nine Judges, Two Effective Votes" (arXiv:2605.29800) —
correlación promedio INCLUSO entre familias distintas (peor par: Claude-Gemini
φ=0.603) — el techo real es `n_eff_max ≈ 1/0.39 ≈ 2.6`, sin importar cuántos nodos
se armen. Coincide con la recomendación del paper: agregar jueces más allá de 5 da
beneficio marginal negligible.

## Aplicado a mmorch

Con 1 sola familia de verificador activa hoy (Google), los "nodos" propuestos
colapsarían en la práctica a la misma fuente de sesgo (σ²_N ≈ σ²_G) → n_eff≈1,
mismo fallo que el panel same-model ya descartado, con más capas de costo. NO se
construyó la infraestructura de nodos — el lever real sigue siendo activar más
familias (Kimi). Un panel PLANO de 3 familias reales ya se acerca al techo
n_eff≈2.5-3 sin necesitar jerarquía; la jerarquía solo empieza a pagar con bastante
más de 3 familias activas repartidas entre nodos, y ahí el paper mismo dice que el
beneficio marginal ya es chico.

## Decisión

Gate de evolve queda como está (`_diff_only_adds_guards` determinístico + 1 llamado
LLM honesto). Revisitar esta pregunta cuando haya ≥3 familias de verificador activas
simultáneamente — recién ahí vale la pena medir φ̄ real entre ellas antes de invertir
en arquitectura de nodos.
