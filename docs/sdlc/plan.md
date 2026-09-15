# plan.md

## Archivos

- `mmorch/code_embedder.py` [R1, R4] [P]
- `mmorch/memory.py` [R2, R3, R4]

> Orden: `mmorch/code_embedder.py` primero (no depende de nada del plan); `mmorch/memory.py` después porque importa `code_embedder`.

### `mmorch/code_embedder.py` [R1, R4]

1. **Conservar** el docstring de módulo y los imports existentes; agregar `from typing import Any, Literal` y el import del módulo de pesos tal como ya lo usa el repo (p. ej. `from . import weights`), sin tocar otros archivos.
2. **Estado de caché**: declarar
   `_CACHE: tuple[Any, Any] | Literal[False] | None = None`
   con `None` = no cargado, `False` = pesos no verificados, `tuple` = `(modelo, vocab)` verificados.
3. **`_load() -> tuple[Any, Any] | None`**:
   - Si `_CACHE is False` → `return None` (no volver a llamar a `weights.verify`).
   - Si `isinstance(_CACHE, tuple)` → `return _CACHE`.
   - Si `_CACHE is None`:
     - `ok, _motivo = weights.verify('code_embedder')`; si `not ok` → `_CACHE = False`; `return None`.
     - si `ok`: `npz = weights.resolve('code_embedder')`; `vocab_path = weights.card('code_embedder')['vocab_path']` (resolver relativo a `weights.ROOT` con `Path(weights.ROOT) / vocab_path`); cargar modelo y vocab (misma función de carga que ya usa el módulo para texto); `_CACHE = (modelo, vocab)`; `return _CACHE`.
4. **`available() -> bool`**: `return _load() is not None`.
5. **`embed_code(text: str) -> list[float] | None`**: `loaded = _load()`; si `loaded is None` → `return None`; si no, `modelo, vocab = loaded`, embeber `text` con `modelo`/`vocab` y devolver `list[float]` (sin inferir cuando `_load()` es `None`).
6. Dejar el camino existente de texto intacto (mismo comportamiento byte a byte) — R4.

### `mmorch/memory.py` [R2, R3, R4]

1. **Conservar** el docstring de módulo y todo el comportamiento actual de `kind='text'`; solo agregar el import de `code_embedder` (p. ej. `from . import code_embedder`) y ramas para `kind == 'code'`.
2. **`write_note(scope, text, *, kind='text', path)`** [R2]:
   - Si `kind == 'code'`: `vec = code_embedder.embed_code(text)`.
     - `vec is not None` → persistir `emb_model='code_embedder'`, `dim=384` y el vector.
     - `vec is None` → persistir embedding `NULL` (mismo camino que texto sin modelo), sin fallar.
   - Si `kind != 'code'` → rama actual sin cambios (bge o `NULL`).
3. **`recall(query, scope, *, kind='text', k, track, path)`** [R3]:
   - Si `kind == 'code'`: `qvec = code_embedder.embed_code(query)`; si `qvec is None` → `return []`; en el rerank fino, filtrar candidatos a `emb_model == 'code_embedder'` únicamente.
   - Si `kind == 'text'`: comportamiento actual, pero el rerank fino compara SOLO candidatos con `emb_model != 'code_embedder'`.
   - No mezclar espacios: los dos conjuntos de candidatos son disjuntos.
4. **`kind` por defecto `'text'`** en ambas funciones; `Note` sigue siendo la clase existente con `id: int`, `text: str` y demás campos — no redefinirla [R4].
5. No tocar tests ni otros archivos; conservar firmas y valores por defecto [R4].

## Prueba

`python -m pytest tests/test_sdlc_d13_code_embedder_recall.py -q`