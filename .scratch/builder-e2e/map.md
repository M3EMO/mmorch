# Mapa wayfinder — Builder de punta a punta con oráculo agente
Label: wayfinder:map · Creado: 2026-10-13 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Fijado por el ticket 01 (2026-10-15). El builder es una herramienta GENERAL: arma entregables (productos, servicios, flujos, funcionalidades) y cada uno queda medido como banco de mmorch. El mapa termina cuando el builder arma de punta a punta la calculadora de sueldo neto de F2 con cinco condiciones:

1. La capa computable queda verde: casos de neto, agente que maneja el preview, consola sin errores, Lighthouse y metadatos.
2. El usuario interviene solo en dos paradas (walkthrough y preview final), con menos de 1 h en total.
3. El gasto de APIs queda debajo de US$5.
4. El run-log registra coder sorteado, fricción, USD, cupo y horas del usuario.
5. El juez en sombra deja su opinión con rúbricas, sin bloquear.

Detalle completo: `brainstorms/2026-10-14-builder-e2e-destino.md`.

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

- [Destino y alcance](issues/01-destino-y-alcance.md) — herramienta general medida como banco; primer entregable F2 sueldo neto; termina en el preview verificado; capa computable bloquea y el juicio con rúbricas va en sombra; walkthrough como Artifact y dos paradas humanas; APIs US$5; Claude en descomposición, planner y agente del preview; coder Haiku o DeepSeek por sorteo.

- [Inventario: qué del SDLC sirve por módulo y qué falta](issues/02-inventario-sdlc.md) — el SDLC sirve para el módulo 2 en adelante; bloquean el bootstrap del repo vacío, la descomposición (`division.md` sin consumidor) y el encadenado entre módulos (cada corrida parte del HEAD).

## Not yet specified

- Oráculo por tipo de entregable (producto, servicio, flujo, funcionalidad): ticket 03.
- Haiku 5.5 como coder: rutearlo por la CLI de Claude, como el planner `claude:<modelo>`; la familia anthropic no entra al REGISTRY.
- `human_interventions` del SDLC nunca se incrementa: contarlo para medir las horas del usuario.
- Tope de US$5 por entregable que sume todos sus módulos.
- Quién descompone en módulos y con qué contrato entre módulos.

## Out of scope

- Modificar juegos: eso es universal-modder, clonado aparte en el escritorio.
- Agente de publicación (hosting, dominio, AdSense): esfuerzo aparte, con gate humano por zona roja.
- Segundo entregable de otro tipo: destino del mapa siguiente.
