# Plan: Hermeticidad de `_codegraph_context` (sin autoindex sorpresivo)

## Archivos
- `mmorch/project_loop.py` [R1, R2, R3] [P]

## Cambios

Unico archivo modificado. No se tocan tests ni otros archivos. Se conserva el docstring del modulo.

### `_codegraph_context(repo: str, tarea: str) -> str`

Implementar el orden de evaluacion unico y comun:

1. Resolver `repo = Path(repo).resolve()`.
2. Si existe `<repo>/.codegraph`: ir al camino R3 (comportamiento actual, primera `_cg` con `sub == "sync"`), **sin** evaluar R1 ni R2.
3. Si no existe, leer `os.environ.get("MMORCH_CODEGRAPH_AUTOINDEX", "0")`.
4. Si el valor no es exactamente `"1"`: **R1**, devolver `""` sin invocar `_cg` ni `shutil.which`, sin correr `init`/`index`, sin crear ni modificar nada en el repo (en particular no crear `.codegraph`).
5. Si es `"1"`: comparar con `is_relative_to` el path resuelto de `repo` contra `Path(tempfile.gettempdir()).resolve()`; si cae dentro: **R2**, devolver `""` sin invocar `_cg`.
6. Solo si nada corta antes: correr `init` e `index` y devolver el contexto.

`CODEGRAPH_BIN` sigue resolviendose via `shutil.which` sin cambios.

### Docstring de `_codegraph_context` [R1, R2]

Declarar explicitamente que el default de `MMORCH_CODEGRAPH_AUTOINDEX` es `0` (sin opt-in no se corre `init`/`index`) y que un repo bajo `tempfile.gettempdir()` no se indexa ni con opt-in.

## Prueba
`python -m pytest tests/test_sdlc_d11_codegraph_hermetico.py -q`