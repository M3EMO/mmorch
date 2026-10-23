# Haiku 5.5 como coder por sorteo
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo entra Haiku 5.5 como coder sin romper el invariante del REGISTRY (la familia anthropic no es un nodo)?

- Ruta: la CLI de Claude, como el planner `claude:<modelo>` de `sdlc/pipeline.py`.
- Sorteo por módulo, mitad y mitad, con semilla fija, entre Haiku 5.5 y DeepSeek v4 pro.
- El run-log guarda el coder sorteado, la fricción, el éxito, los dólares y el cupo.

## Answer

Resuelto (2026-10-15). Un coder `claude:<modelo>` corre por la CLI de Claude, sin pasar por el REGISTRY (la familia anthropic sigue sin ser un nodo).

- `sdlc/pipeline.py`: `llm()` desvía `claude:<modelo>` a `_claude_texto()`, la misma llamada aislada del planner (`--restricted`, sin MCP, sin sesión), pero sin herramientas (`--tools ""`) y con el rol del coder como `--system-prompt`. El costo queda en `claude_coder_usd` y el coder en `state["coder"]`.
- `sdlc/builder.py`: `sortear()` reparte DeepSeek v4 pro y `claude-haiku-5-5` mitad y mitad (±1), mezclados con semilla fija. El plan puede cambiar `coders` y `semilla`. Cada fila registra el coder y su cupo.
- Tests: `tests/test_sdlc_coder_claude.py` y `tests/test_builder.py`.

Corrida real con sorteo (F2, `prototipo-10`, semilla 20261015):

| Módulo | Coder | Resultado | Cupo del coder |
|---|---|---|---|
| m1 parámetros | Haiku 5.5 | built, 1,7 min | US$0,0015 equiv. |
| m2 cálculo | DeepSeek v4 pro | built, 2,6 min | — |
| m3 interfaz | DeepSeek v4 pro | escalate: 6 rechazos de compilación y un clon entre `index.html` y `src/app.ts` | — |

El builder cortó en m3, como corresponde. DeepSeek construyó m3 en las corridas de los tickets 04 y 06: el mismo coder varía entre corridas. Con n = 1 módulo por Haiku no hay comparación; el banco necesita muchos entregables.
