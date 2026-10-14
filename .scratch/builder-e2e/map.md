# Mapa wayfinder — Builder de punta a punta con oráculo agente
Label: wayfinder:map · Creado: 2026-10-13 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Provisorio, lo fija el ticket 01. Una decisión respaldada por bancos sobre la arquitectura de un builder que arma un producto desde una idea, módulo por módulo. El veredicto de cada módulo y del producto combina tests computables con evidencia que un agente junta al manejar el producto real. Un prototipo arma de punta a punta un producto chico de varios módulos.

## Notes

- Origen: revisión de `rehan-remade/universal-modder` (2026-10-13). Su ciclo es recon → ruta → laboratorio → rebanada vertical → assets → verificación en el juego real → nota de campo. Veredicto de la revisión: no sirve para mmorch como código; sirve como patrón.
- Pedido del usuario: un builder de punta a punta, no solo con tests computables, capaz de armar desde cero de forma modular.
- Invariante que manda (GOAL.md, CLAUDE.md global): lo checkeable va a un oráculo, nunca a un LLM. El "oráculo agente" se parte en dos roles:
  - El agente maneja el producto y junta evidencia: capturas, logs, respuestas HTTP y salidas de la CLI.
  - Aserciones deterministas sobre la evidencia dictan el veredicto computable.
  - Un verificador separado, con razonamiento prendido, juzga solo el resto no computable.
  - Ese verificador empieza en sombra, como el juez visual de `gate_sprites` (`mmorch/sdlc.py:462`), hasta que una medición lo habilite.
- Debilidad de universal-modder que no se copia: el agente que escribe el mod también lee la captura y dicta el veredicto.
- Lo que ya existe: la skill `/project` con el SDLC de 6 etapas (`mmorch/sdlc.py`), modo grill para la aceptación, gates deterministas y fix ladder.
- Lo que falta: descomposición desde cero en módulos, oráculo de integración entre módulos, agente que maneja el producto y memoria entre corridas.
- Insumo en bd: `orchestration-4rf` mide notas de campo de corridas previas en el SDLC. Primaria: rechazos de gates y escalones del fix ladder; secundaria: éxito por ítem.
- Dato previo (`orchestration-34k`): en corridas reales el planner ya acierta 0.875 por ítem; el cuello de botella es el coder de la etapa 4.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Inventario: qué del SDLC sirve por módulo y qué falta](issues/02-inventario-sdlc.md) — el SDLC sirve para el módulo 2 en adelante; bloquean el bootstrap del repo vacío, la descomposición (`division.md` sin consumidor) y el encadenado entre módulos (cada corrida parte del HEAD).

## Not yet specified

- Tipos de producto que cubre el builder (web, CLI, API, app de escritorio, juego).
- Quién descompone en módulos y con qué contrato entre módulos.
- Costo por producto: cupo de Claude contra API barata contra pool de Cursor.

## Out of scope

- Modificar juegos: eso es universal-modder, clonado aparte en el escritorio.
