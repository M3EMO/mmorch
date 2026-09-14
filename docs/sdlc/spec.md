# Spec: Robustez de entrada malformada en server_fleet y server_pty

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

### `mmorch/server_fleet.py`

- `async def fleet_handler(request: Request) -> JSONResponse`
- `async def fleet_run(request: Request) -> JSONResponse`

### `mmorch/server_pty.py`

- `async def pty_open(request: Request) -> JSONResponse`
- `async def pty_resize(request: Request) -> JSONResponse`
- `async def pty_input(request: Request) -> JSONResponse`

`Request` y `JSONResponse` se importan de `starlette.requests` y `starlette.responses`. El helper `_token_ok(request: Request) -> bool` se conserva y se invoca primero en todos los handlers; si devuelve `False`, se responde `401` con `{"error": ...}`. No se agregan dependencias nuevas.

## Requisitos

- R1: En todo handler de `mmorch/server_fleet.py` y `mmorch/server_pty.py` que invoque `await request.json()`, si el body no parsea (`JSONDecodeError`/`ValueError`) o el resultado no es un `dict`, se responde `400` con `{"error": "body JSON invalido"}`. Orden: `_token_ok` -> 401; en `pty_resize` y `pty_input`, existencia de sesión -> 404; luego parseo del body -> 400.
- R2: En `pty_open` y `pty_resize`, si `rows` o `cols` no son convertibles a `int` (falla `int()` con `ValueError`/`TypeError`), se responde `400` con `{"error": "rows/cols deben ser enteros"}`. Orden: tras parsear body como `dict` y antes de abrir/redimensionar.
- R3: En `fleet_run`, si `forward` devuelve un `dict` con `ok` False y su `error` indica que el host no está registrado, se responde `404` con el mismo mensaje `error` devuelto por `forward`; si `ok` es False por otro motivo, se responde `502` con ese `error`; si `ok` es True, se responde `200` con el resultado. Orden: tras parsear body y extraer `host`, `path`, `payload`.
- R4: En `pty_resize` y `pty_input`, la verificación de existencia de la sesión (`sid` de la ruta) ocurre antes de `await request.json()`; si la sesión no existe, se responde `404` con `{"error": ...}` aunque el body sea inválido. Orden: `_token_ok` -> 401, sesión -> 404, body -> 400, lógica.

## Casos borde

- R1: body no JSON (`b"{esto no es json"`), JSON que no es objeto (lista, número, string, `null`), body vacío.
- R2: `rows` string no numérico (`"abc"`), `rows` float no entero (`1.5`), `rows` `null`, `cols` ausente o no entero.
- R3: host desconocido (`"no-existe"`), host registrado pero `forward` devuelve `ok` False por otro error, `forward` devuelve `ok` True.
- R4: sesión inexistente con body no JSON (`b"[["`), body JSON inválido, body válido pero sesión inexistente.

## Criterios de exito

- R1: `POST /fleet` con `content=b"{esto no es json"` y token válido -> `400` y `{"error": "body JSON invalido"}`.
- R2: `POST /pty/open` con `json={"rows": "abc", "cols": 80}` y token válido -> `400` y `{"error": "rows/cols deben ser enteros"}`.
- R3: `POST /fleet/run` con `json={"host": "no-existe", "path": "/state", "payload": {}}` y token válido -> `404` y `{"error": <string devuelto por forward>}`.
- R4: `POST /pty/nope/resize` con `content=b"[["` y token válido -> `404` y body JSON con clave `error`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_fleet_body_no_json_es_400 |
| R2 | test_R2_pty_open_rows_no_numerico_es_400 |
| R3 | test_R3_fleet_run_host_desconocido_es_404 |
| R4 | test_R4_pty_resize_sesion_inexistente_body_roto_es_404 |

## Clarificaciones

- P1: El Contrato nombraba `fleet` pero el codigo define `fleet_handler`. ¿A que funcion se aplica R1? | R: `fleet_handler`. El test `test_R1_fleet_body_no_json_es_400` postea a la ruta `/fleet`, que mapea a `fleet_handler` en `mmorch/server_fleet.py`. Se corrigio el nombre en la seccion Contrato. | afecta: R1