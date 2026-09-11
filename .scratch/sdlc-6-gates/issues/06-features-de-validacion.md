# Features de validacion
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Elegir el set de validación: 3 features reales del mapa de QueTePario (tickets 13-18, `Desktop/QueTePario/ChatBot/docs/agents/wayfinder/issues/`) con test de aceptación ejecutable, de tamaño chico/medio/grande, más 2 tareas sintéticas del bench con oráculo. Para cada una: punto de partida en git (main ya tiene Spring y el webhook), comando de aceptación, y por qué ese tamaño. Salida: `research/06-features.md`. AFK: lo hace el agente; el usuario revisa la lista.

## Answer

Revisado 2026-09-11 tras corrección del usuario: el mapa es SOLO del SDLC en mmorch; el ChatBot es un proyecto aparte y sus features no entran acá. Detalle en [research/06-features.md](../research/06-features.md).
- 3 del bench congelado (`mmorch/bench.py`): S1 `lru-ttl-cache` (chica, held-out, se corre una vez al final), S2 `rate-limiter` (mediana, control: el engine viejo midió 0/3), S3 `etl-pipeline` (grande, interfaces entre módulos).
- 3 de dogfood, features de mmorch con pytest: D1 `gen_model` se propaga a la recursión; D2 gate `test-compile` en `project_integrate` como primer gate del contrato del ticket 02; D3 detector de atasco (hash repetido 2 veces → salta de nivel), pedido del usuario 2026-09-11.
- Punto de partida: el repo que `materialize` crea (bench) o `main` de orchestration (dogfood). Los tests de aceptación los escribe Claude; el pipeline no toca `tests/`.
- Orden: S2 → D1 → D3 → S3 → D2 → S1. Costo total estimado < US$3.
- Fuera: el ChatBot. Si el pipeline sirve, el ChatBot lo usa desde su mapa, con su `sdlc.toml`.
