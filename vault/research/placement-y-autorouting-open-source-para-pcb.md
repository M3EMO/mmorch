---
title: Placement y autorouting open-source para PCB
created: 2026-09-24
tags: [research, pcb-builder, research, placement, autorouting, eda]
status: verified
confidence: 0.8
sources: [https://github.com/freerouting/freerouting, https://github.com/freerouting/freerouting/blob/master/docs/command_line_arguments.md, https://github.com/freerouting/freerouting/discussions/508, https://github.com/freerouting/freerouting/issues/575, https://raw.githubusercontent.com/freerouting/freerouting/master/docs/scoring.md, https://api.github.com/repos/freerouting/freerouting/releases/latest, https://dev-docs.kicad.org/en/apis-and-binding/pcbnew/index.html, https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/index.html, https://docs.kicad.org/kicad-python-main/index.html, https://gitlab.com/kicad/code/kicad/-/blob/master/LICENSE.README, https://en.wikipedia.org/wiki/Pcb-rnd, https://manpages.ubuntu.com/manpages/stonking/man1/route-rnd.1.html, https://github.com/devbisme/skidl, https://devbisme.github.io/skidl/, https://github.com/michaelgale/pcbflow, https://github.com/atopile/atopile, https://github.com/tscircuit/tscircuit, https://github.com/tscircuit/tscircuit-autorouter, https://github.com/tscircuit/tscircuit-autorouter/pull/2449, https://github.com/tscircuit/autorouting, https://github.com/ajokela/pyplacer, https://github.com/enzo-tf/dodplace, https://github.com/The-OpenROAD-Project-Attic/SA-PCB, https://en.wikipedia.org/wiki/Lee_algorithm, https://arxiv.org/abs/2204.01006, https://freerouting.org/freerouting/using-with-kicad]
---
---
mision: Decidir si pcb-builder construye placement/autorouting propio o envuelve herramientas open-source existentes.
verifier: fuentes primarias (GitHub API, docs oficiales), sin verificador LLM
---

## Tronco

Envolver: freerouting (CLI headless, DSN->SES) para ruteo y un placer SA propio corto (tipo pyplacer, ~1.4k lineas) para placement; KiCad queda como formato de intercambio via kicad-python (IPC) y no como motor.

## Que es

Relevamiento de herramientas y librerias open-source que hacen placement o autorouting de PCB. Fecha: 2026-09-24. Datos de mantenimiento tomados de la API de GitHub (`pushed_at`) el mismo dia.

## Evidencia / mecanismo

### Autorouting

| Herramienta | Licencia | Lenguaje | Invocacion | Calidad reportada | Mantenimiento |
|---|---|---|---|---|---|
| freerouting | GPL-3.0 | Java | CLI headless (`java -jar freerouting.jar --gui.enabled=false -de in.dsn -do out.ses -mp N -mt N`), REST API v1 self-hosted (Docker `ghcr.io/freerouting/freerouting`), cliente `pip install freerouting-client`, MCP server | Score propio 0-1000 (completion + clearance), corpus PCBench 1157 fixtures. Regresion conocida v2.x vs v1.9 (discussion #508): v2.x dejo 50 nets sin rutear en tablero de 300 conexiones. DRC interno mas laxo que el de KiCad (issue #575). Reglas de KiCad no siempre se transfieren (foro KiCad, 04/2025) | Activo. Ultimo push 2026-09-24. Release v2.4.1 el 2026-09-03. Refactor 2026 completo |
| tscircuit-autorouter (`@tscircuit/capacity-autorouter`) | MIT | TypeScript | Libreria npm. `new AutoroutingPipelineSolver(simpleRouteJson)`, loop `step()` hasta `solved`, `getOutputSimpleRouteJson()`. Entrada SimpleRouteJson (capas, obstaculos rectangulares, conexiones) | Dashboard propio. Dataset01 85/85; SRJ18 81.3% completion+DRC; SRJ33 con 37 casos fallidos abiertos. Benchmark vs freerouting (PR #2449): ambos timeout a 180 s en tablero de 138 nets, inconcluso | Activo. Ultimo push 2026-09-24. 2673 commits, 85 stars |
| tscircuit/autorouting (dataset) | no declarada | TypeScript | CLI `autorouting-dataset benchmark`. Incluye A* de ejemplo (`infinite-grid-ijump-astar`) | Solo benchmark | Archivado 2025-08-15 |
| route-rnd (Ringdove) | GPL-2 | C | CLI `route-rnd -m metodo -o out.tedax in.tedax`. Entrada y salida tEDAx | Sin numeros publicos | Version 0.9.3. Sitio repo.hu inaccesible el 2026-09-24 |
| KiCad | GPL-3 (codigo bajo Boost/GPL segun LICENSE.README) | C++ | Sin autorouter desde KiCad 5. Solo router interactivo push-and-shove | n/a | Activo |

### Placement

| Herramienta | Licencia | Lenguaje | Invocacion | Calidad reportada | Mantenimiento |
|---|---|---|---|---|---|
| pyplacer | BSD-3 | Python 3.10+, sin dependencias | CLI `python3 run.py in.kicad_pcb out.kicad_pcb --iterations --cooling --seed`. Lee y escribe `.kicad_pcb` de KiCad 9 | SA con costo HPWL + solape + fuera de borde + congestion + salida de pad. Sin rotacion. Autor reporta 93.8% ruteo exitoso (mejor de 64 seeds) vs 99.4% Quilter.ai en shield Arduino Giga denso | Un solo commit, 2026-04-24. 0 stars. ~1400 lineas |
| dodplace | Apache-2.0 | C + front Python/KiCad | Libreria. ePlace/DCT analitico + Hungarian + SA | Sin benchmark publicado | Creado 2026-09-24. 21 commits. 0 stars. Demasiado nuevo |
| SA-PCB (OpenROAD attic) | BSD-3 | C++ | CLI `./sa -i -j -t -p in -f out`. Formato Bookshelf | SA con solape analitico de poligonos y rotacion | Archivado 2026-07-01 |
| KiCad pcbnew SWIG API | GPL-3 | Python | `pcbnew.LoadBoard`, `SetPosition`, `SaveBoard`. Funciona standalone sin GUI | Sin placement automatico. Es solo lectura/escritura de tablero | Deprecado desde KiCad 9. Se elimina en KiCad 11 |
| KiCad IPC API + `kicad-python` | GPL-3 | Python (protobuf + NNG) | `pip install kicad-python`. Requiere instancia de KiCad corriendo. Mueve footprints, agrega tracks, corre DRC | Sin placement automatico. Sincrono, una conexion por instancia | Oficial. Estable desde KiCad 9.0. Solo editor PCB |

### Generadores de diseño (no hacen placement ni routing de PCB)

| Herramienta | Licencia | Lenguaje | Que hace | Mantenimiento |
|---|---|---|---|---|
| skidl | MIT | Python | Netlist desde codigo. `generate_schematic()` hace placement force-directed y routing, pero solo de esquematico. Para PCB delega a KiCad | Activo. Ultimo push 2026-09-24 |
| pcbflow | BSD-3 | Python | Placement manual por coordenadas y routing "turtle" manual. Export Gerber. Sin autorouting | Ultimo push 2025-02-12. Sin releases |
| atopile | MIT | Python + Zig | Lenguaje `.ato`, compilador, `ato build`. Layout delegado a KiCad | Ultimo push 2026-06-13 |
| tscircuit | MIT | TypeScript | Circuitos en React. CLI `tsci dev`. Usa `@tscircuit/capacity-autorouter` y `@tscircuit/autolayout` (solo esquematico) | Activo. Ultimo push 2026-09-23 |

### Algoritmos publicados

- Placement SA: Sechen y Sangiovanni-Vincentelli (1985). pyplacer y SA-PCB lo implementan. Es el estandar de facto para tableros chicos.
- Placement force-directed: Quinn y Breuer (IEEE 1979, "A forced directed component placement procedure for printed circuit boards"). Survey Cheong y Si, arXiv 2204.01006 (2022). Rapido pero produce solapes; sirve como inicializacion antes de SA.
- Placement analitico: ePlace/DCT. Solo dodplace lo implementa en PCB y es de hoy.
- Routing Lee (1961): BFS por onda. Optimo si existe camino. Lento y caro en memoria.
- Routing A*: Lee con heuristica. tscircuit lo usa en grillas infinitas. Es la base de un router simple propio.
- Routing push-and-shove y ripup-retry: lo que hace freerouting. No es trivial de reimplementar.

## Aplicable a mmorch

- No aplica a mmorch. Nota de proyecto pcb-builder.

## Objeciones

- La calidad de freerouting v2.x tiene una regresion documentada respecto a v1.9. Falta medir en tableros propios de pcb-builder con `-mp` fijo y comparar score.
- pyplacer tiene un solo commit y cero stars. Su numero de 93.8% viene del autor y no esta replicado.
- tscircuit-autorouter tiene 37 fallos DRC abiertos. La comparacion contra freerouting quedo inconclusa por timeouts.
- route-rnd no se pudo verificar: repo.hu estaba caido.

## Veredicto cross-family

- No corrido. Fuentes primarias verificadas a mano.

## Veredicto construir vs envolver

1. Routing: envolver freerouting. Es el unico autorouter open-source maduro con CLI headless y formato estandar (DSN/SES). Fijar version (v2.4.1) y medir con su score.
2. Alternativa de routing: `@tscircuit/capacity-autorouter` si el stack es TypeScript. Entrada JSON simple, sin Java. Menos maduro.
3. Placement: construir. Ningun placer open-source esta mantenido y probado. Un SA sobre HPWL + solape cabe en ~1000 lineas (pyplacer es referencia BSD-3 para copiar ideas).
4. Formato: usar `.kicad_pcb` como intercambio. Leer y escribir con `kicad-python` (IPC) o parser propio del S-expression. No depender del SWIG (muere en KiCad 11).
5. No envolver skidl, pcbflow, atopile ni pcb-rnd para placement/routing: ninguno lo hace.

## Links
- [[pcb-builder]]
