# Convención por repo (la arquitectura de proyectos)
Type: grilling
Status: resolved
Blocked by: 02
Map: ../map.md
Capture: `brainstorms/2026-09-10-sdlc-6-gates.md` (Q14–Q16b)

## Question

Qué lleva cada repo para ser robusto de una.

## Decisions

- Sin `accept_cmd` en `sdlc.toml` el repo **no entra**.
- Obligatorio: `docs/sdlc/gates/` (aceptación + test-compile + 4 del A/B), plantillas intent/spec/plan, puntero en `AGENTS.md`.
- Opcional: hooks, ETS, Lotus, CI YAML. No bloquean el engine.
- `contract_version`: engine rechaza si no la entiende. Repo viejo corre. Knobs del producto = `comando`/`entrada`.
- `/new-project` crea esa lista. No copia el engine. No arma Lotus ni CI.
- Intent y PR: sin ficha. No se inventa gate.
- Ejemplo real: ChatBot, cuando `accept_cmd` esté lleno.
- Tope USD/minutos: ticket 07.
