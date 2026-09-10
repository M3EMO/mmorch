# Features de validacion
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Elegir el set de validación: 3 features reales del mapa de QueTePario (tickets 13-18, `Desktop/QueTePario/ChatBot/docs/agents/wayfinder/issues/`) con test de aceptación ejecutable, de tamaño chico/medio/grande, más 2 tareas sintéticas del bench con oráculo. Para cada una: punto de partida en git (main ya tiene Spring y el webhook), comando de aceptación, y por qué ese tamaño. Salida: `research/06-features.md`. AFK: lo hace el agente; el usuario revisa la lista.

## Answer

Set elegido (detalle, aceptación y por qué en [research/06-features.md](../research/06-features.md)):
- Punto de partida real: `ChatBot` `main` @ `77bc70c` (Spring + matcher + webhook, 13/13 verde). El baseline del A/B ya no sirve.
- F1 chica: Menú configurable (`Opcion`) → `Bot`, deuda del ticket 13. Test: `MenuConfigurableTest` (H2).
- F2 mediana: Reserva persistida, ticket 16 sin Mercado Pago. Test: `ReservaAcceptanceTest` por `ConversacionService` + API.
- F3 grande: Conector WooCommerce Store API con fixtures reales, ticket 15. Test: `WooSyncAcceptanceTest` sin red.
- S1 `rate-limiter` y S2 `lru-ttl-cache` del bench congelado (`mmorch/bench.py`, `materialize`). S1 es control: el engine viejo midió 0/3.
- Dashboard (14) queda afuera: su aceptación es un flujo en navegador y no hay e2e en el repo.
- Los 3 tests de aceptación los escribe Claude antes de lanzar; el pipeline no los toca (gate baseline-intacto sobre `src/test`).
- Orden: F1 → S2 → F2 → S1 → F3. Costo total estimado < US$5.
Pendiente del usuario: confirmar la lista.
