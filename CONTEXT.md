# mmorch

Orquestación determinista multi-modelo que trata el **cupo** del plan Claude como el recurso escaso, no el dólar.

## Language

**Cupo**:
Cuota del plan Claude Code. Lo que el conductor se niega a gastar en bulk.
_Avoid_: presupuesto en USD (eso es BudgetKeeper), tokens

**Conductor**:
Python determinista (`mmorch`) más el orquestador humano/Opus. No es un nodo de la orquesta.
_Avoid_: agente, músico, worker

**OneFlow**:
En una tarea subjetiva, generador y verificador son de familias distintas; si serían el mismo modelo, es un solo agente.
_Avoid_: multi-agente homogéneo, self-check, debate same-family

**Checker**:
Verificador determinista (código, cero API) sobre una afirmación computable.
_Avoid_: LLM-judge, verifier (cuando hay ground truth)

**Judge / verificador**:
Escéptico LLM, familia distinta al autor, refuta por default. El acuerdo no confirma.
_Avoid_: reviewer amable, self-score como label

**Zona**:
Radio de explosión × reversibilidad (verde / azul / amarillo / rojo). Rojo nunca es autónomo.
_Avoid_: severity, P0

**Fitness**:
Batería que un cambio auto-propuesto tiene que pasar antes de PR (`evaluate` + git sandbox). No es un LLM diciendo “se ve bien”.
_Avoid_: score de confianza, thumbs-up
