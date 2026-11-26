# Compresor de arranque: qué partes y con qué palanca
Type: grilling
Status: resolved
Blocked by: 01, 02, 06
Map: ../map.md

## Question

Con el desglose medido del arranque, decidí con el usuario qué fuentes se comprimen primero. Decidí también la palanca de cada una: reescribir, recortar descripciones, `skillOverrides` o mover a carga bajo demanda. La regla "dejar ECC igual" vale para todo el ticket.

## Answer

Resuelto con el usuario el 2026-11-26 en un grilling. Captura completa: `vault/brainstorms/2026-11-26-vault-y-compresor-compresor-de-arranque.md`.

- Una medición nueva corrige el ticket 02. Claude Code lista casi todas las skills de ECC sin descripción, así que ECC pesa ~16.5k, no 43k. El listado de skills mide 12.9k, los agentes 9.5k y los `.md` propios 6.8k.
- La meta sigue en 52k o menos, con varias palancas (usuario: "52k, varias palancas"). La meta bajó después a 55k (usuario, 2026-11-26).
- Primera palanca: `skillOverrides` pone en `user-invocable-only` las 392 skills de ECC sin uso en el mes (~-5.6k).
- Segunda palanca: la receta SkillReducer recorta las descripciones de las 12 skills propias. `skillOverrides` oculta las skills de Claude Code sin uso, salvo `artifact-*`, `docs`, los browsers y `claude-api` (~-2.5k).
- Tercera palanca: se apagan Gmail, flint-chart, scheduled-tasks y mcp-registry, sin llamadas en el mes (~-1.3k).
- Los hooks de ECC y de ponytail quedan prendidos (usuario: "Ninguno, ya esta.").
- Cuarta palanca: el compresor reescribe los CLAUDE.md, AGENTS.md y MEMORY.md propios.
- Riesgo cerrado: con un recorte de 40% en los `.md`, el arranque estimado queda en ~54.7k. La meta de 52k pedía ~79%, fuera del rango medido. El usuario bajó la meta a 55k el 2026-11-26 ("Sí, 55k").

