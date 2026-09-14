## Archivos

- `mmorch/mcp_server.py` [R1, R2, R3, R4, R5] [P]

## Detalle de la edición (`mmorch/mcp_server.py`, único archivo tocado)

No se modifican tests, `mmorch/paths.py` ni `pyproject.toml`.

1. Imports nuevos a nivel módulo, todos **antes** de la línea `mcp._mcp_server.version = ...`:
   - `import importlib.metadata` (R4: obliga a resolver `version` como atributo del módulo en tiempo de llamada).
   - `import tomllib`.
   - `from mmorch.paths import repo_root`.
   - Prohibido `from importlib.metadata import version`.

2. Función pública a nivel módulo, definida **antes** de la asignación de versión:

   ```python
   def mmorch_version() -> str:
   ```

   - Sin parámetros posicionales ni keyword; retorno `str` no vacío; nunca propaga excepciones.
   - Consulta siempre con el nombre literal `"mmorch"`.
   - Rama 1 (R1, R4): dentro del cuerpo, `importlib.metadata.version("mmorch")`; si no lanza, se retorna ese valor. Es la primera rama evaluada (acceso vía módulo, no símbolo importado, para que el parcheo sea observable).
   - Rama 2 (R2): si la rama 1 lanza `PackageNotFoundError`, leer con `tomllib` desde `repo_root() / "pyproject.toml"` y retornar el valor de la clave `version` de la tabla `[project]`. Solo cuenta `[project]` (no `[tool.*]`). Es la segunda rama evaluada.
   - Rama 3 (R3): si la rama 2 no obtiene un valor válido —archivo ausente, TOML inválido o ilegible, sin tabla `[project]`, sin clave `version`, o valor no `str`— retornar exactamente `"0.0.0"`. Es la tercera rama evaluada y la última. Cubrir toda excepción de I/O/parseo con `try/except` que caiga aquí, sin propagar.
   - Estructura de control: `try`/`except PackageNotFoundError` para R1→R2 y un `try`/`except Exception` interno para R2→R3, de modo que ninguna ruta escape como excepción.

3. Asignación a nivel módulo, **después** de la definición de `mmorch_version()` y de la creación de `mcp` (R5):

   ```python
   mcp._mcp_server.version = mmorch_version()
   ```

   - Garantiza que al importar `mmorch.mcp_server`, `mcp._mcp_server.version` sea truthy e igual al valor de `mmorch_version()` (peor caso `"0.0.0"`).

Orden interno obligatorio: imports → `mmorch_version()` → creación de `mcp` → asignación de `version`.

## Prueba

`python -m pytest tests/test_sdlc_d5_version.py -q`