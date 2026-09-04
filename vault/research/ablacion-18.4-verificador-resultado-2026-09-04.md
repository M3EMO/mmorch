---
applies_to:
- orchestration
date: 2026-09-04
status: applied
tags:
- ablation
- cross-family
- verification
- methodology
title: Ablación §18.4 — el verificador importa muchísimo, y el modo de falla es el falso-rechazo
---

# Ablación §18.4 — resultado real (n=327, pareado, McNemar)

Primera corrida de esta ablación cuyo resultado **queda guardado**. La del 2026-06-08
(1.417 llamadas) se fue por stdout; ver `corpus-training-no-tiene-senal-de-routing-2026-09-04.md`.

## Setup

Gold set de 350 ítems paramétricos con **ground truth computada** (factoriales,
combinatorias, potencias, sumas): ~50% correctos, ~50% con error inyectado determinista.
Etiqueta infalible, sin humano y sin LLM. El **mismo** ítem lo juzgan los dos
verificadores (diseño pareado) → McNemar sobre pares discordantes. Seed 42, reproducible.

Costo real: **US$ 0,135**. 23/350 pares descartados por fallo de API (7%) → análisis
sobre 327 pares completos.

## Resultado

| verificador | sensibilidad (caza-bug) | especificidad (no falso-rechazo) | acc. balanceada |
|---|---|---|---|
| `deepseek-chat` (SELF) | 0,939 IC95[0,891–0,967] | **0,558** IC95[0,482–0,632] | 0,749 |
| `gemini-2.5-flash` (CROSS) | 1,000 IC95[0,977–1,0] | **0,982** IC95[0,947–0,994] | 0,991 |

**McNemar:** b=0, c=79, χ²=77,01, p≈0. En los 79 pares discordantes ganó CROSS en **los
79**. SELF no ganó ninguno.

## Dónde está la diferencia (importa más que el titular)

No es que un verificador "vea más": los dos cazan los bugs casi perfecto (0,94 vs 1,00).
**Toda la brecha está en la especificidad**: `deepseek-chat` **rechaza el 44% de las
respuestas CORRECTAS**. `gemini-2.5-flash` rechaza el 2%.

El modo de falla del verificador barato no es dejar pasar errores — es **refutar trabajo
bueno**. Eso encaja con el ~74% de false-refute de jueces LLM en checkable difícil que ya
teníamos medido: es el mismo fenómeno, ahora con diseño pareado y ground truth infalible.

## Qué NO prueba esto (el confound sigue en pie)

- **Es un modelo por familia.** No se puede separar "familia distinta" de "mejor en
  aritmética". El resultado establece que **la elección del verificador importa
  enormemente** (0,75 vs 0,99 sobre los mismos ítems), no que la familia sea la causa.
  Para atribuirlo a familia haría falta ≥2 modelos por familia, cruzados.
- **No mide auto-endoso.** El error es *inyectado*, no propio del verificador: acá no se
  testea el punto ciego generativo ("un modelo no ve SU error"). El docstring del script
  lo dice explícitamente.
- **Aritmética sintética.** Detección de errores numéricos inyectados, no calidad
  subjetiva. Angosto a propósito: es lo que permite la etiqueta infalible.

## Consecuencia operativa inmediata

`deepseek-chat` es el modelo más usado del sistema (12.093 llamadas) y el generador por
defecto. Donde se lo use para **verificar** afirmaciones computables está rechazando ~44%
del trabajo correcto: reintentos y refutaciones desperdiciados, y ruido metido en el
corpus de entrenamiento como si fuera señal.

Corolario que ya sabíamos y ahora está cuantificado: **donde hay un checker determinista,
usarlo** (`checkers.py` tiene 21 y solo se usó uno, 278 de 21.046 llamadas). Un LLM
juzgando aritmética computable es caro y peor.

## La corrida rota que vino antes (vale como hallazgo)

El primer intento de hoy dio sensibilidad 1,0 / especificidad 0,0 en **ambos** brazos y
**cero pares discordantes en 348 ítems** — imposible entre modelos distintos salvo salida
constante.

Causa: `_parse_verdict` fue endurecido el 2026-08-28 (commit 3897345) para que el formato
legacy `{"passed": bool}` **sólo pueda refutar** — fix de seguridad correcto contra un
verificador hijackeado. La ablación, escrita el 2026-06-08, seguía pidiendo ese formato.
El parser rechazaba toda aprobación por diseño: se medía el arnés, no a los modelos.

Cronología: **la respuesta existía en junio, se tiró por no persistirla, y cuando se
volvió a buscar el instrumento ya estaba roto por un cambio ajeno de 11 semanas después.**

Mitigación: `tests/test_ablation_protocol.py` — 3 tests a US$0 (sin API) que fallan si el
contrato entre el prompt de la ablación y el parser vuelve a divergir. La lección no es
sólo "persistir resultados": **un arnés de experimentos que no corre seguido se pudre en
silencio**, y el cambio que lo rompe no tiene cómo enterarse.
