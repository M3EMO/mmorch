# plan.md

## Archivos
- `mmorch/canal.py` [R1, R2, R3, R4] [P]

## Detalle

**`mmorch/canal.py`** — único archivo tocado. En `post(...)` (firma existente, sin cambios de nombres ni parámetros nuevos):

1. Como **primera operación** dentro de `post()`, antes de resolver `path` con `canal_path()`, antes de leer/abrir/escribir: calcular `body.strip()` sobre el parámetro `body` y, si el resultado es `""`, lanzar `ValueError('body vacio')`. [R1, R2]
2. Al disparar la validación no se debe resolver ni tocar `path`/archivo: ni crear el archivo inexistente, ni agregar/quitar líneas si existe — el archivo queda idéntico al estado previo. [R3]
3. Para `body.strip() != ""` se mantiene el comportamiento actual **sin cambios**: agregar el registro al canal y devolver el `dict` con `rec["body"] == body.strip()` (se conserva el `strip()` existente; no se agrega normalización). [R4]

No se agregan, renombran ni eliminan funciones públicas del módulo. No se modifican tests ni otros archivos.

## Prueba

`python -m pytest tests/test_sdlc_12249f76b2.py -q`