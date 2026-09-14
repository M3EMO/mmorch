# Spec: version determinista de mmorch para el servidor MCP

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.

## Contrato

Archivo modificado (unico): `mmorch/mcp_server.py`. No se tocan tests, `mmorch/paths.py` ni `pyproject.toml`.

Imports nuevos a nivel modulo, todos antes de la asignacion de `mcp._mcp_server.version`:

```python
import importlib.metadata
import tomllib
from mmorch.paths import repo_root
```

- `importlib.metadata`, `tomllib`: stdlib de Python 3.12, sin dependencias externas.
- `repo_root()`: funcion ya existente en `mmorch/paths.py`, devuelve `pathlib.Path`.
- Prohibido `from importlib.metadata import version`.

Funcion publica, a nivel modulo en `mmorch/mcp_server.py`:

```python
def mmorch_version() -> str:
```

- Sin parametros posicionales ni keyword. Retorno `str` no vacio. Nunca propaga excepciones.
- Consulta la metadata siempre con el nombre literal `"mmorch"`.
- Definida antes de la linea de asignacion.

Asignacion a nivel modulo, despues de la definicion de `mmorch_version()` y de la creacion de `mcp`:

```python
mcp._mcp_server.version = mmorch_version()
```

## Requisitos

- R1: `mmorch_version()` retorna el resultado de `importlib.metadata.version("mmorch")` cuando esa llamada no lanza; es la primera rama evaluada.
- R2: si `importlib.metadata.version("mmorch")` lanza `PackageNotFoundError`, `mmorch_version()` retorna el valor de la clave `version` de la tabla `[project]` leida con `tomllib` desde `repo_root() / "pyproject.toml"`; es la segunda rama evaluada.
- R3: si la rama de R2 no obtiene un valor (archivo ausente, TOML invalido o ilegible, sin tabla `[project]`, sin clave `version` o valor no `str`), `mmorch_version()` retorna exactamente `"0.0.0"`; es la tercera rama evaluada y la ultima.
- R4: la metadata se resuelve como atributo del modulo en tiempo de llamada (`importlib.metadata.version("mmorch")` dentro del cuerpo de `mmorch_version`), de modo que el parcheo de `importlib.metadata.version` sea observable.
- R5: al importar `mmorch.mcp_server`, `mcp._mcp_server.version` queda asignado con el retorno de `mmorch_version()`: no vacio e igual al valor de `mmorch_version()`.

## Casos borde

- R1: nombre consultado distinto de `"mmorch"` o valor retornado por la metadata vacio -> no aplica la primera rama tal cual; el string vacio se retorna solo si la metadata lo devuelve.
- R2: git worktree sin metadata instalada (`PackageNotFoundError`) -> se usa la version del `pyproject.toml`.
- R2: `pyproject.toml` con `version` fuera de `[project]` (por ejemplo en `[tool.*]`) -> no cuenta; solo la tabla `[project]`.
- R3: `repo_root() / "pyproject.toml"` inexistente -> `"0.0.0"`.
- R3: TOML malformado o archivo no legible -> `"0.0.0"` sin excepcion propagada.
- R3: `[project]` sin clave `version`, o `version` no string -> `"0.0.0"`.
- R4: parche aplicado sobre el objeto modulo `importlib.metadata` antes de la llamada -> el efecto del parche se observa dentro de `mmorch_version()`.
- R5: import del modulo con metadata ausente -> `mcp._mcp_server.version` no vacio (peor caso `"0.0.0"`).

## Criterios de exito

- R1: `importlib.metadata.version` parcheado a `lambda name: "9.9.9-test"` -> `mmorch_version() == "9.9.9-test"`.
- R2: `importlib.metadata.version` parcheado para lanzar `PackageNotFoundError("mmorch")` -> `mmorch_version()` igual al primer match de `(?m)^version\s*=\s*"([^"]+)"` en `repo_root() / "pyproject.toml"`.
- R3: sin pyproject accesible o sin `[project].version` -> `mmorch_version() == "0.0.0"`, sin excepcion.
- R4: `mmorch_version()` responde al parche de `importlib.metadata.version` aplicado sobre el modulo (cubierto por las entradas de R1 y R2).
- R5: tras importar `mmorch.mcp_server`, `mcp._mcp_server.version` es truthy y `mcp._mcp_server.version == mmorch_version()`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_mmorch_version_usa_metadata_cuando_existe |
| R2 | test_mmorch_version_cae_a_pyproject_sin_metadata |
| R3 | test_mmorch_version_cae_a_pyproject_sin_metadata |
| R4 | test_mmorch_version_usa_metadata_cuando_existe, test_mmorch_version_cae_a_pyproject_sin_metadata |
| R5 | test_server_expone_version_no_vacia |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.