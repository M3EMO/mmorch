# Ticket 06 — Set de validación del pipeline (3 features reales + 2 sintéticas)

Fecha: 2026-09-10. Criterio de selección: test de aceptación ejecutable sin red ni credenciales,
tamaño distinto, y que cada una toque una parte del pipeline que el A/B no ejercitó.

## Punto de partida en git (las 3 reales)

`QueTePario/ChatBot`, rama `main` en `77bc70c` ("Ticket 13 resuelto: H1 backend en main, suite 13/13").
Ya tiene Spring Boot 3.3, JPA con H2 en perfil test, el matcher del brazo B mergeado, el webhook
de Meta y la API del dashboard. Comando de aceptación común: `cd backend && mvn -q -B test`.
La suite completa (13 tests) es el gate de regresión; el test nuevo de cada feature es la aceptación.
El baseline del A/B (`d6853c9`) queda atrás: no sirve, no tiene Spring.

Regla para el pipeline: el test de aceptación de cada feature lo escribo YO antes de lanzar,
como en el A/B. El pipeline no puede tocarlo (gate `baseline-intacto` sobre `src/test`).

## F1 — chica: Menú configurable → Bot (deuda declarada del ticket 13)

- Qué: `BotFactory` construye el `Bot` con el menú del `Negocio` (`Opcion` con `titulo`, `orden`, `accion`, `hijos`) en vez del menú fijo del prototipo. Sin opciones cargadas, cae al menú fijo.
- Tamaño: 2-4 archivos (`Bot` acepta un menú; `BotFactory` lo arma; `ApiController` invalida caché al editar).
- Aceptación: `MenuConfigurableTest` (H2): Negocio con 3 opciones ordenadas → `Bot.MENU` del bot de ese negocio lista esas 3 en orden; Negocio sin opciones → menú fijo; editar una opción por la API invalida la caché.
- Por qué: es el primer cambio sobre código que el pipeline no escribió. Mide si el coder respeta un contrato existente (`Bot` con 571 líneas ya en main).

## F2 — mediana: Reserva persistida (ticket 16, sin Mercado Pago)

- Qué: entidad `Reserva` (negocio, cliente, producto, variante, entrega, localidad, estado, link), repositorio, `ConversacionService` persiste cuando el bot cierra una reserva (`Reply.order()` no nulo), endpoint `GET /api/reservas` y `PUT /api/reservas/{id}/estado`. El link de pago es un stub `https://mpago.la/demo-{n}`, como en el prototipo.
- Tamaño: 5-7 archivos, toca dominio, servicio, web y test.
- Aceptación: `ReservaAcceptanceTest` (H2): flujo por `ConversacionService.recibir` "zuecos rayadas 40" → "si" → "retiro" crea una Reserva PENDIENTE con los datos; `GET /api/reservas` la lista con Basic auth; `PUT` la pasa a CONFIRMADA; 13 tests previos siguen verdes.
- Por qué: cruza 3 capas y un contrato existente (`Reply.order()` como string). Es donde el engine viejo falló (12 unidades, `integration_failed` por interfaces).

## F3 — grande: Conector de catálogo WooCommerce Store API (ticket 15, con fixture)

- Qué: `WooStoreConnector` que lee `wp-json/wc/store/v1/products` y `?type=variation` (el mismo esquema que `prototypes/demo-pitch/demo.py` ya parsea), mapea a `Producto`/`Variante` del Negocio, upsert idempotente por id externo, `POST /api/negocio/{id}/sync` que lo dispara, y `BotFactory.invalidate` al terminar.
- Tamaño: 6-9 archivos, cliente HTTP, mapeo, persistencia, endpoint, test con fixture.
- Aceptación: `WooSyncAcceptanceTest` (H2, sin red): un `HttpClient` fake sirve `fixtures/woo-products.json` + `fixtures/woo-variations.json` (recortes reales de quetepario.com, guardados en el repo) → 12 productos y sus variantes en DB con precio, fotos y stock; segunda sync no duplica; el bot del negocio ve el stock ("tenes borcegos en 38?" responde con el producto real). 13 previos verdes.
- Por qué: es el "servicio de adecuación" del modelo de negocio, con oráculo real (el prototipo ya lo hace en Python). Y es la feature más grande sin credenciales.

## S1 y S2 — sintéticas del bench congelado de mmorch

`mmorch/bench.py` ya tiene tasks multi-módulo con tests de aceptación congelados (anti-Goodhart:
exigen comportamiento). Se materializan con `materialize(task, dst)` en un repo git nuevo.

- S1 `rate-limiter`: la task donde el engine viejo midió 0/3 con el mismo planner. Sirve como control: si el pipeline la pasa, es mejora contra un número histórico.
- S2 `lru-ttl-cache`: tamaño chico, para medir el piso de costo/tiempo del pipeline en algo trivial.
- `etl-pipeline` queda como held-out si hace falta validar que lo aprendido generaliza.

## Qué se mide por feature (ticket 07 lo fija; esto es la propuesta)

| métrica | fuente |
|---|---|
| aceptación verde / rojo | `mvn test` o `pytest` a mano, no el run-log |
| USD | `logs/metrics.jsonl` por fase (`ab-sdlc-<feature>`) |
| minutos de pared | run-log |
| rechazos por gate y vueltas | run-log `gates` y `test_rounds` |
| nivel máximo de escalación alcanzado | run-log (`reasoner_rounds`, `escalated_to_claude`) |
| intervenciones de Claude | `supervision.md` |

Brazos: pipeline v3 (0 Claude) vs híbrido (Claude en escalación). El engine viejo ya no se corre:
el ticket 05 lo reemplaza, y el A/B ya lo midió.

## Orden propuesto

F1 → S2 → F2 → S1 → F3. Chico primero para ver el piso, grande al final. Costo estimado total:
menos de US$5. Tiempo: ~1 h de pipeline, más lo que yo tarde en escribir los 3 tests.

## Lo que el usuario decide

- Confirmar las 3 features o cambiar alguna por la del dashboard (ticket 14). Esa no entra hoy:
  su aceptación es un flujo en el navegador y no hay Playwright ni e2e en el repo.
- Confirmar que escribo yo los tests de aceptación (como en el A/B) y que el pipeline no los toca.
