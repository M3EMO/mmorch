## Archivos

- `mmorch/synth_store.py` [R1, R2, R3, R4] [P]

Reemplaza la ancla basada en `__file__`/`parents[1]` por el import de modulo `from .paths import repo_root` y la constante `STORE_PATH = repo_root() / "synth_checkers.json"`, evaluada en tiempo de import, sin escrituras de estado. Unico archivo modificado (no se tocan `mmorch/paths.py`, `tests/test_paths.py` ni otros). Al no depender de otros archivos del plan, el item es paralelizable.

## Prueba

`python -m pytest tests/test_paths.py -q`