# Spec: validación de `body` vacío en `mmorch/canal.py::post()`

## Contrato

- Archivo modificado: `mmorch/canal.py` (único archivo tocado; no se modifican tests ni otros archivos).
- `mmorch/canal.py::post(src: str, kind: str, body: str, *, artifacts: list[str] | None = None, verify_cmd: str = "", verify_expect: str = "", to: str = "", path: Path | None = None) -> dict` — firma ya existente, sin cambios de nombres ni parámetros nuevos.
  - `path` es keyword-only: los tests lo pasan siempre como `path=p`. Default `None`, resuelto internamente con `canal_path()` (no existe una constante `DEFAULT_PATH`).
  - Retorna el registro escrito como `dict`, con la clave `"body"` igual al resultado de `body.strip()` — el mismo `strip()` que ya aplica el código actual antes de guardar; sin cambios adicionales de normalización.
  - Lanza `ValueError('body vacio')` cuando `body.strip() == ""`.
- No se agregan, renombran ni eliminan funciones públicas del módulo.

## Requisitos

- R1: `post()` evalúa la validez de `body` como primera operación, antes de resolver `path`, leer, abrir o escribir; si `body == ""` lanza `ValueError('body vacio')`.
- R2: si `body.strip() == ""` —incluye `"   "`, tabs, saltos de línea y cualquier mezcla de whitespace— lanza `ValueError('body vacio')`; es el mismo predicado de R1, evaluado antes de cualquier efecto sobre el archivo.
- R3: cuando R1 o R2 disparan, `post()` no escribe: no crea el archivo si no existe y no agrega ni quita líneas si existe; el archivo queda idéntico a su estado previo.
- R4: si `body.strip() != ""`, `post()` conserva el comportamiento previo sin cambios: agrega el registro al canal y devuelve el `dict` con `rec["body"]` igual a `body.strip()`, tal como ya lo hace el código actual.

## Casos borde

- R1: `body=""` (cadena de longitud 0) con `path` inexistente: no se crea el archivo.
- R2: `body="   "`, `body="\t"`, `body="\n"`, `body=" \t\n "` — todos lanzan `ValueError('body vacio')`.
- R3: canal con historia previa (`"ping previo"` ya escrito) más `body` solo-espacios: el conteo de `"\n"` no cambia.
- R4: `body=" ping "` (texto con espacios alrededor): se escribe `rec["body"] == "ping"` (comportamiento previo: `strip()` ya existente).
- R4: `body="ping"` con `path` inexistente: se crea el archivo y se escribe el registro.
- R4: `path` apunta a un directorio existente (`tmp_path`) y el archivo `.jsonl` no existe todavía: el caso válido sigue creando el archivo.

## Criterios de éxito

- R1: `post("cursor", "status", "", path=p)` → lanza `ValueError` con `str(exc) == "body vacio"`.
- R2: `post("cursor", "status", "   ", path=p)` → lanza `ValueError` con `str(exc) == "body vacio"`.
- R3: con `p` conteniendo 1 línea (`count("\n") == 1`), `post("cursor", "status", "  ", path=p)` → lanza `ValueError` y `p.read_text(encoding="utf-8").count("\n") == 1`.
- R4: `rec = post("cursor", "status", "ping", path=p)` → `rec["body"] == "ping"` y `p.exists() is True`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_body_vacio_lanza_valueerror |
| R2 | test_R2_body_solo_espacios_lanza_valueerror |
| R3 | test_R3_body_vacio_no_agrega_linea |
| R4 | test_R4_body_con_texto_sigue_funcionando |

## Clarificaciones

- P1: ¿La firma de `post()` usa `autor`/`tipo`/`DEFAULT_PATH` como decía el Contrato, o los nombres reales del código (`src`, `kind`, `canal_path()`)? | R: los nombres reales del código actual; la spec tenía nombres inventados que no existen en `mmorch/canal.py`. Fuente: código actual, la TAREA no pide renombrar nada. | afecta: Contrato
- P2: R4 pedía guardar el `body` "sin normalizar", pero el código actual aplica `body.strip()` antes de guardar y la TAREA dice que un body con texto "sigue funcionando igual". ¿Se mantiene el `strip()` existente? | R: sí, se mantiene el `strip()` existente; "sigue funcionando igual" significa no tocar esa lógica. | afecta: R4