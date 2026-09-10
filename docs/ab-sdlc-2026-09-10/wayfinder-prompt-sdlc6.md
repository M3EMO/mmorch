Chartear el mapa wayfinder de "SDLC de 6 etapas con gates" en mmorch. Repo: ~/.claude/orchestration. Tracker: docs/agents/issue-tracker.md. Mapa nuevo, no tocar los existentes.

## Destino
mmorch construye una feature de un repo real con un pipeline de 6 etapas (intent → spec → plan → build → test → PR), donde cada gate de salida es determinista o un checker sintetizado del synth_store, y Claude entra solo por escalación (N fallos seguidos) dejando siempre una corrección y un chequeo nuevo. Medido: aceptación verde, USD, minutos e intervenciones por feature, contra el /project actual.

## Evidencia (no re-derivar; leer antes de resolver tickets)
- A/B/C del 2026-09-10 sobre QueTePario/ChatBot (port del matcher demo.py a Java, 35 asserts, oráculo Python):
  - A = /project engine: 4 intentos, rojo; 4 defectos del engine encontrados y corregidos (commits 58ce334, ea8b775): planner convertía el test de aceptación en unidad; coder ciego a sus deps; deepseek-chat + max_fix=1 por regex; max_tokens 16384 vacía la salida de v4-pro. Intento 5 en curso.
  - B = 6 etapas automático (scratchpad ab_sdlc_b.py): verde, 31 llamadas, $1.13, 22 min, 0 intervenciones. El v1 se rompió a sí mismo: fix loop reescribía 9 archivos; v2 = el modelo nombra archivos + compile gate con revert.
  - C = 6 etapas híbrido: verde, 9 llamadas, $0.16, 9 min, 2 intervenciones de Claude (revisión de plan 3 min; `Catalog.load` con IOException no manejada, 1 método). Las dos se reemplazan por gates: `mvn test-compile` y dos regex sobre plan.md.
  - Resultados: scratchpad ab_results.json; worktrees ChatBot-abB (ab/b-sdlc f9798ad) y ChatBot-abC (ab/c-hybrid dcdef5c); docs/sdlc/{intent,spec,plan,supervision}.md.
- Checkers sintetizados (README "Measured", vault "Checkers sintetizados por tipo"): deepseek-reasoner 114/114 funciones correctas por $0.05; synth_store con 57 checkers; 969 ítems juzgados a $0; promoción = 3 ítems + 1 de borde.
- Especificidad de una cadena = su peor gate. Apilar gates buenos es gratis; uno malo envenena todo.
- Dos modelos sin razonamiento de familias distintas escribieron la MISMA función mal (cadenas_sin_ab). Cross-family no decorrelaciona por sí solo; razonamiento y método sí.

## Lo que el mapa tiene que resolver
1. Contrato de gate como código: clase (determinista / sintetizado / juicio), comando, entrada, salida, qué pasa si falla. Un GATE-N.md por etapa. Gate sin chequeo ejecutable = no entra.
2. Gates nuevos que salen del A/B: test-compile entre build y test; plan solo toca archivos permitidos; baseline intacto; firmas del contrato coinciden con el test.
3. Fix dirigido como estándar del engine: el modelo nombra archivos, compile gate por vuelta, revert si rompe.
4. Escalación a Claude: cuándo (N fallos), qué recibe (log, oráculo, diff), qué deja (corrección + chequeo nuevo en synth_store). Registro en supervision.md.
5. Sintetizador de checkers = deepseek-reasoner; coder = deepseek-v4-pro; ningún gate LLM sin número medido.
6. Integrar B (driver) al engine de /project o reemplazarlo: decidir con los 4 fixes ya hechos.
7. Validación: 3 features más del ChatBot (tickets 13-18 del mapa de QueTePario) por el pipeline, con USD/min/intervenciones contra 1 etapa. Si no gana, decirlo.
8. Lo subjetivo: cómo un gate de juicio produce ejemplos etiquetados (3 buenos + 3 malos) y baja a sintetizado.

## Fuera de alcance
Dashboard, WhatsApp, multi-tenant (son del mapa de QueTePario). GOAL.md no se toca. Nada de esto se generaliza a tareas subjetivas sin ítems medidos ahí.

## Reglas
Resolver tickets es HITL. Refutar por default; ambiguo = no concluyente. Research al vault global con tag mmorch. Avisar antes de superar ~$15 en un paso.
