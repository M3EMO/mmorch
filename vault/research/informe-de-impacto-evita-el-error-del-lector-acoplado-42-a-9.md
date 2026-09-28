---
title: Informe de impacto evita el error del lector acoplado: 42% a 96% (2026-09-25)
created: 2026-09-25
tags: [research, mmorch, impacto, acople-por-datos, agentes, codegraph, benchmark, wayfinder]
status: measured
confidence: media
sources: [.scratch/mapa-de-impacto (tickets 01, 02, 09); worktree ../orch-spike rama spike/impacto, scripts/tareas, tareas_ts, tareas_java, tareas_indirectas, tareas_externos, tareas_portfolio y scripts/impacto; video youtube k2qls2LiBRc]
---
## Pregunta
El video "AI gives too much code" describe un error: un agente cambia un escritor y no actualiza a un lector acoplado por datos, sin referencia en el codigo. ¿Un informe de impacto automatico lo evita?

## Protocolo (pre-registrado, ticket 09)
- 8 mini-repos Python con 30 modulos señuelo cada uno; 12 tareas: 8 acopladas (el cambio correcto exige tocar al lector) y 4 de control.
- Oraculo: tests ocultos de escritor y lector. Validado: la referencia completa pasa; la referencia "solo escritor" falla en las 8 acopladas y pasa en las 4 de control.
- Agente: una accion JSON por turno (list_files, read_file, grep, write_file, run_tests, done), 15 pasos.
- A = solo herramientas. B = herramientas + informe estatico: literales del archivo escritor (tablas SQL, rutas, claves JSON, variables de entorno) que aparecen en otros modulos.
- 3 repeticiones por tarea y condicion. Un revisor cross-family (deepseek-chat) borro dos frases de enunciado que insinuaban al lector.

## Resultado
| Modelo | Acopladas A -> B | Control A -> B | p (Fisher, una cola) | Costo |
|---|---|---|---|---|
| deepseek-v4-pro (principal) | 42% -> 96% (10/24 -> 23/24) | 100% -> 100% | 4.4e-05 | US$0.81 |
| deepseek-chat (exploratorio) | 42% -> 79% (10/24 -> 19/24) | 100% -> 100% | 8.6e-03 | US$0.03 |

- Sin informe, las 14 fallas de cada modelo son el error del video: cambio el escritor y nunca toco al lector.
- Con informe, v4-pro toco al lector en 24/24 tareas acopladas.
- Las 5 fallas de deepseek-chat con informe ignoraron el informe; el ruido de señuelos probablemente ahoga la señal.
- Piloto: con frases que insinuaban al lector, A acertaba casi siempre. Sin ellas, t01 A = 0/3. Los agentes revisan lectores acoplados solo cuando alguien les avisa.

## Contexto del mapa (mismo dia)
- Ticket 01 (negativo): la seleccion de tests por imports ya da recall 1.00 en mmorch; la metrica por tests que fallan no ve escapes.
- Ticket 02: codegraph 0.9.6 no ve datos (~89 almacenes), indirectas (~104 funciones como valor) ni ejecucion (~26). Su reindex incremental borra en cascada los llamadores entrantes del archivo editado; 1.6.0 lo corrige y resuelve alias de modulo.

## Limites
- Claude escribio repos y tareas; el tamaño del efecto vale para esta clase de tarea.
- El extractor solo ve literales compartidos; no ve rutas calculadas ni despacho dinamico.
- Control facil: el costo de sobre-editar quedo poco exigido.

## Implicacion
Un informe de literales compartidos, inyectado antes de editar, cierra la mayor parte del error del lector acoplado con un modelo capaz. El ruido del informe importa para modelos baratos. Siguiente: formato del informe (ticket 03) y herramienta integrada (ticket 05).

## Actualizacion 2026-09-26 — replicas, instalacion y limites
Todas con el mismo protocolo: tareas acopladas y de control, tests ocultos, oraculo validado (referencia completa pasa; "solo escritor" falla en acopladas y pasa en control), deepseek-v4-pro, 3 repeticiones, criterio pre-registrado (+25 puntos en acopladas sin perder mas de 10 en control).

| Replica | Acopladas A -> B | Control A -> B | p | Costo |
|---|---|---|---|---|
| Python sintetico (ticket 09) | 42% -> 96% | 100% -> 100% | 4e-5 | US$0.81 |
| TypeScript sintetico (ticket 11), informe v1 | 38% -> 100% | 100% -> 100% | 1e-6 | US$1.21 |
| Java sintetico (ticket 12) | 38% -> 96% | 100% -> 100% | 1.4e-5 | US$0.87 |
| Aristas indirectas en Python (ticket 04), literales solo -> + indirectas | 38% -> 92% | 100% -> 100% | 9e-5 | US$1.11 |
| Lector en prompt o hook de agente (ticket 13), + lectores en configuracion | 0% -> 100% | 100% -> 100% | 4e-7 | US$0.78 |
| Repo sin tests ni run_tests (ticket 14), informe instalado | 92% -> 88% | 100% -> 100% | - | US$0.81 |
| Cursor: informe despues de la primera edicion (ticket 10) | 42% -> 79% | 100% -> 100% | 9e-3 | US$0.36 |
| **Portfolio, codigo real del usuario (ticket 07)** | **54% -> 96%** | 100% -> 92% | 9e-4 | US$5.18 |

- Portfolio: en t01, t04 y t07, sin informe fallo 9/9 y con informe acerto 9/9; sin informe, el agente nunca toco al lector.
- Alcanza con senalar al modulo que comparte datos: en TypeScript, v1 acerto aunque no viera el campo exacto (solo el nombre de archivo compartido).
- Aristas indirectas (ticket 04): el informe de literales no ve funciones pasadas como valor. Sin ayuda fallan kwarg, `getattr` con prefijo y atributo asignado; lista de hooks, dict de funciones y `key=` las resuelve el grep del agente. Un salto alcanza salvo en registros por decorador (2/3).
- Lectores fuera del repo (ticket 13): los lectores externos reales de orchestration son prompts (tarea programada nocturna, skills) y hooks, no codigo de otros repos. Sin avisar, el agente nunca los toca (0/12); con el nombre del archivo y la ruta del prompt, siempre (12/12). El indice de literales entre repos hermanos da puro ruido en repos reales.
- Sin tests (ticket 14): quitar tests visibles y run_tests casi no cambia el acierto (22/24 -> 21/24); el agente llega al lector por el informe.
- Leccion operativa: el hook importaba el `__init__` del paquete (openai, 8-30 s) y superaba su corte de 15 s; medir el hook como subproceso, no solo la funcion.
- Limite visto al portar a Java: si el escritor no tiene literales (el nombre del archivo vive en una clase de acceso a datos), el informe sale vacio. No medido.
- Una linea extra "tests que importan el modulo" no mejoro (20/24 vs 23/24, p=0.17) y se quito: los tests visibles no cubren a los lectores.
- Instalado: `mmorch/impacto.py` + hook `PreToolUse` en Claude Code y `postToolUse` en Cursor; Python, TypeScript y Java (tree-sitter opcional); en Python suma aristas indirectas (`mmorch/impacto_indirecto.py`) y lectores en prompts y configuracion de agentes (`mmorch/impacto_externo.py`), ambos con cache. Sin `.git` en ningun padre, el hook no informa.

## Negativos y lecciones de medicion
- Co-cambio en git como verdad (Portfolio, orchestration, Proyecto_Adepor): recall 0.12-0.20 (con el informe que mira la edicion) y lift 12-104x sobre el azar. El informe tiene señal, pero un co-cambio no implica "el lector tenia que actualizarse": verdad debil.
- Banco sobre repo real v1: invalido por efecto piso (15 pasos; write_file reescribia archivos recortados a 6000 caracteres). Con edit_file, lectura por rango y 30 pasos, el control subio a 100%.
- Una corrida murio por falta de saldo en DeepSeek (402 + limite de concurrencia atado al saldo): todo resumen de banco tiene que contar errores de API.
- codegraph medido: para llamadores, grep -w gana en recall (0.83 vs 0.74) y la union con codegraph 1.6.0 baja los "cero llamadores" falsos (68 -> 45); `codegraph explore` para navegar no ahorro tokens (+22%, mismo acierto).
