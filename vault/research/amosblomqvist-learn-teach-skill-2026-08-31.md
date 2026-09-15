---
title: amosblomqvist/learn — skill "teach" (probe → plan → teach)
mision: ¿Qué tiene este sistema de enseñanza con IA que el LLM Wiki (ACTOR + wiki-study) no tenga ya, y vale la pena adoptar?
status: verified
confidence: 0.85
verifier: lectura directa del código fuente (repo clonado, 6k líneas), sin cross-family — es un análisis de diseño, no un claim empírico
tags: [research, estudio]
sources:
  - https://github.com/amosblomqvist/learn
  - https://www.youtube.com/watch?v=kzcI5F4tGiU
created: 2026-08-31
---

## Tronco

Un solo archivo de 146 líneas (`skills/teach/SKILL.md`) codifica una pedagogía en dos principios — verdades incondicionales primero, y "cómo podría haberlo descubierto yo" — ejecutada como probe (bisección del borde del entendimiento) → plan (DAG de dependencias en mermaid) → teach (por nodo: motivar → establecer → conectar → quiz-check); todo lo demás del repo es plomería para el harness `pi`.

## Qué es

Config personal de `pi` (harness estilo Claude Code de earendil-works). Contenido:

- `skills/teach/SKILL.md` — la filosofía y el proceso. **Es el 90 % del valor.**
- `skills/visualize/SKILL.md` — cuándo un diagrama vale la pena; delega a subagentes.
- `extensions/quiz.ts` (1062 líneas) — tool de pregunta calificada: opciones + `correctAnswer` por value (no por posición) + `explanation` obligatoria + shuffle por default + opción "I don't know" **siempre presente y distinta de "wrong"** + campo de nota libre por respuesta.
- `extensions/ask-user-question.ts` — hermano no calificado (preferencias, sin respuesta correcta).
- `extensions/md-log.ts` — espeja la sesión a un `.md` de Obsidian (Obsidian como UI).
- `agents/researcher.md` — fact-check antes de afirmar algo incierto. `agents/mermaid-maker.md`, `svg-maker.md` — renderizan **y miran** el PNG antes de devolverlo.

### Los dos principios

1. **Verdades incondicionales primero.** No por rigor lógico sino porque son lo más fácil de aceptar sin hedging: nada más fundamental va a contradecirlas, así que el cerebro las commitea al instante. Distingue *verdad incondicional* (cómo se sostiene el hecho: sin caveats) de *axioma* (dónde está en el grafo: sin aristas entrantes). Formas fuertes: enunciados universales ("TODA comunicación entre computadoras es {enviar paquetes}") y definiciones reales.
2. **"¿Cómo podría haberlo descubierto yo?"** Un hecho sin razón visible de por qué *tenía* que ser así se siente arbitrario y no se commitea. Cada paso motivado: por qué este problema, por qué esta manipulación. Referencia explícita: 3Blue1Brown.

Mecanismo subyacente que ambos atacan: **el cerebro no commitea un hecho que no está seguro de que sea seguro sostener** (si algo más fundamental puede contradecirlo después, el update sería caro → hedging → el hecho nunca aterriza).

### El proceso

**Probe** — mapear el borde del entendimiento con `quiz`, no spot-check. Regla central: **el borde solo está localizado cuando está acotado por ambos lados** — algo que acierta (piso) y algo que falla (techo) en cada hilo del que dependerá la lección. Corolarios explícitos:
- Todo-correcto ≠ listo: significa que las preguntas eran fáciles. Escalar bruscamente hasta que algo rompa.
- Un solo error ≠ listo ni ≠ "empezar a enseñar": es una coordenada. Sondear alrededor para caracterizarlo (descuido / laguna aislada / concepto erróneo sistemático).
- Bisección: acierta → saltar difícil; falla → cerrar el intervalo.
- Objetivo de aprendizaje se pregunta con `ask_user_question` (sin respuesta correcta); nivel se mide con `quiz`. Frontera limpia entre los dos tools.

**Plan** — con nivel y objetivo, razonar el camino. Dispara `researcher` para mapear el campo. Emite (a) prosa del enfoque, (b) **DAG en mermaid**: verdades incondicionales en las raíces, objetivo como sumidero. El DAG *es* el orden de enseñanza. Stress-test de raíces: ¿es incondicional *para él* o un teorema disfrazado? Si deriva, empujar hacia abajo. **Se detiene y espera el OK** antes de enseñar.

**Teach** — por cada nodo (incondicional o derivado, sin excepción): motivar → establecer → conectar (arista explícita al grafo ya construido) → quiz-check. Un nodo no confirmado es tan peligroso como un hecho no confirmado. No front-loadear fundamentos y dejar de chequear.

### Procedimiento de construcción de opciones de quiz (transferible tal cual)

1. Cada opción es un **claim desnudo, sin justificación**. Toda la razón va en `explanation` (que aparece después de responder). El tell #1 es la correcta llevando su "porque…" y las otras no.
2. **Escribir la correcta primero, mutarla en cada distractor** bajo un concepto erróneo específico, con el mismo esqueleto, grano y registro. Paralelismo por construcción, no por auditoría posterior.
3. Cada distractor es un error real que el alumno podría cometer (diagnóstico: *cuál* elige revela *qué* matiz está mal) pero inequívocamente incorrecto — tentador, no tramposo.
4. Sin negrita asimétrica.

Test: si leyendo las opciones en frío se adivina la correcta sin saber la materia, regenerar, no parchear.

## Evidencia / mecanismo

Sin evidencia empírica en el repo — es un sistema personal "compartido tal cual", sin métricas. Lo que sí tiene:
- Consistencia interna fuerte: cada mecanismo (I-don't-know distinto de wrong, correctAnswer por value, shuffle, distractor diagnóstico) cierra un modo de falla concreto del quiz LLM (adivinar, miscontar posiciones, sesgo posicional, feedback binario pobre).
- Alineación con Dunlosky 2013: probe y quiz-check = practice testing; "motivar cada paso" ≈ elaborative interrogation / self-explanation. No contradice la evidencia; la operacionaliza en un loop de sesión.
- El video muestra una sesión real (formas diferenciales) donde el probe corre largo, el plan aparece como mermaid, y el teach avanza un nodo por mensaje con quiz intercalado.

**Validación local** (2026-08-31, sesión Matemática Hito 1 en el LLM Wiki): sin haber leído este repo, la sesión hizo probe de 16 preguntas → encontró el borde (tabla del condicional invertida) → enseñó ahí → retest → machete. Coincide con la forma probe/teach/quiz-check. Lo que **faltó** y el repo hubiera exigido: (a) tras el retest 4/4, escalar dificultad en vez de avanzar (no se acotó el techo de Lógica), (b) un plan/DAG explícito antes de enseñar, (c) quiz-check por cada nodo enseñado (De Morgan se explicó dos veces sin confirmar en el medio).

## Comparación contra el LLM Wiki (ACTOR + wiki-study + metodos-de-estudio)

| Capacidad | learn | LLM Wiki |
|---|---|---|
| Retrieval practice / quiz | ✅ tool dedicado con grading | ✅ modo 1, modo 5 (recall), app con `derive-tier` |
| **Probe con regla de acotamiento (piso + techo)** | ✅ explícito, central | ❌ modo 1 dice "preguntá y calificá", sin regla de borde |
| **Plan como DAG antes de enseñar** | ✅ mermaid, raíces = incondicionales | ❌ no hay fase plan entre probe y teach |
| **Loop por nodo con quiz-check obligatorio** | ✅ | ⚠️ implícito en coaching, no exigido |
| **Construcción de distractores** | ✅ procedimiento de 4 pasos | ⚠️ app tiene `whyWrong` por opción y `difficulty: trampa` — el problema de calidad de distractores está **abierto** (memoria `project_quiz_distractors`) |
| "No sé" distinto de "mal" | ✅ | ❌ app: `selectedIndex: -1` = sin responder, semántica distinta |
| Fact-check antes de afirmar | ✅ researcher subagent | ✅ `verify-cross` (mmorch), pero no mandado mid-teaching |
| Socrático vs expositivo adaptativo | ✅ por tema y energía | ✅ coaching socrático por default |
| Obsidian como UI | ✅ md-log | ✅ el vault entero |
| Visuales verificadas mirando el render | ✅ subagentes | ⚠️ matplotlib/mermaid sin loop de inspección |
| **Espaciado / repetición en el tiempo** | ❌ single-session | ✅ mastery `repasar`, calendario, Cepeda |
| **Compresión (Tronco/Ramas/Hojas)** | ❌ | ✅ ACTOR-C |
| **Objeciones / Test** | ❌ | ✅ ACTOR-T, sparring |
| **Run con ground-truth (nashpy, sympy)** | ❌ | ✅ ACTOR-R |
| Examen simulado con rúbrica | ❌ | ✅ modo 11 |
| Multi-materia, persistencia entre sesiones | ❌ | ✅ |

**Veredicto**: el LLM Wiki es más ancho (ciclo completo de estudio, persistencia, verificación externa); `learn` es más profundo en **el loop de una sesión de enseñanza**. No son competidores: `learn` es una mejora quirúrgica del modo 1 y 4 de wiki-study.

## Aplicable a mmorch / Estudio

Adopción mínima (4 adiciones a `.claude/skills/wiki-study/SKILL.md`, ~35 líneas, sin skill nueva ni tooling nuevo — Claude Code ya tiene `AskUserQuestion` como quiz funcional):

1. **Modo 1 (quiz) → regla de acotamiento.** El borde requiere piso y techo por hilo. Todo-correcto → escalar. Un error → sondear alrededor antes de enseñar. Bisección.
2. **Procedimiento de distractores** (los 4 pasos) como sección transversal. **Resuelve directamente el TODO abierto de distractores en la app** (`project_quiz_distractors.md`).
3. **Fase plan opcional** para temas nuevos: DAG mermaid de dependencias con incondicionales en raíz, esperar OK. Encaja natural: `trunks.md` ya es compresión por source; el DAG es el camino por sesión.
4. **Modo 4 (deep-dive) → loop por nodo**: motivar → establecer → conectar → quiz-check. Nada se apila sobre un nodo no confirmado.

NO adoptar: harness `pi`, extensión TUI de quiz, md-log (redundantes con Claude Code + vault), subagentes de visuales (el vault ya renderiza; el loop de inspección es la única idea rescatable y es cara).

## Objeciones

- **Sin métricas.** El autor no mide retención ni transferencia; "me funciona hace años" es anecdótico. La adopción se justifica por alineación con Dunlosky y consistencia interna, no por evidencia del repo. El dato que falta: comparar mastery a 2 semanas entre sesiones con/sin regla de acotamiento — el vault tiene la infraestructura (`derive-tier`, `lastSeenAt`, `history`) para medirlo.
- **Probe largo tiene costo.** El video muestra ~10 preguntas antes de enseñar nada. Para un alumno con 40 min post-trabajo eso puede comerse la sesión. Mitigación: acotar el probe a los hilos del objetivo declarado (el skill ya lo dice: "bound by relevance to the goal").
- **"Verdad incondicional" es relativa al alumno**, y el skill lo admite ("¿es incondicional *para él*?"). Eso vuelve el stress-test de raíces dependiente del probe: si el probe falló en un hilo, la raíz elegida puede ser arena. No es un defecto del diseño, es un recordatorio de que las 3 fases están acopladas.
- **Modelo grande requerido.** El autor usa Kimi K3 / Sonnet 5 y dice explícitamente que "la inteligencia del modelo importa mucho" porque las instrucciones son específicas. Delegar el teach a Haiku/DeepSeek (cupo barato vía mmorch) probablemente degrade el paso "motivar". Verificación y distractores sí son delegables.

## Veredicto cross-family

- No corrido (análisis de diseño sobre código leído, no claim empírico). Si se quiere validar la adopción, el experimento es el de la primera objeción, y es medible con datos que el vault ya guarda.

## Links
- [[actor-framework]] — el marco que esto extiende
- `Estudio/general/metodos-de-estudio.md` — Dunlosky, la base de evidencia
- `Estudio/.claude/skills/wiki-study/SKILL.md` — destino de las 4 adiciones
- `Estudio/Matematica ingreso/wiki/exams/Hito 1 - diagnostico y machete.md` — la sesión que lo validó sin saberlo
