# Spec: Cablear `code_embedder` a `memory` con verificación de pesos

## Contrato

### mmorch/code_embedder.py

- Variable de módulo: `_CACHE: tuple[Any, Any] | Literal[False] | None = None`
  - `None`: no cargado.
  - `False`: pesos no verificados (no disponible).
  - `tuple[Any, Any]`: `(modelo, vocab)` cargados y verificados.

- `def _load() -> tuple[Any, Any] | None:`
  - Si `_CACHE` es `False`, retorna `None`.
  - Si `_CACHE` es una tupla, retorna la tupla.
  - Si `_CACHE` es `None`, llama a `verify('code_embedder')`. Si no pasa, asigna `_CACHE = False` y retorna `None`. Si pasa, usa `resolve('code_embedder')` para el .npz y `card('code_embedder')['vocab_path']` (relativo a `weights.ROOT`) para el vocab, carga el modelo, asigna `_CACHE = (modelo, vocab)` y retorna la tupla.

- `def available() -> bool:`
  - Retorna `_load() is not None`.

- `def embed_code(text: str) -> list[float] | None:`
  - Obtiene `(modelo, vocab)` de `_load()`. Si `_load()` retorna `None`, retorna `None`.
  - Si hay modelo, embebe `text` con el modelo y el vocab, y retorna `list[float]`.

### mmorch/memory.py

- `def write_note(scope: str, text: str, *, kind: str = 'text', path: str | Path) -> int:`
  - Si `kind == 'code'`: llama a `code_embedder.embed_code(text)`. Si el resultado no es `None`, guarda `emb_model='code_embedder'`, `dim=384` y el vector correspondiente. Si es `None`, guarda embedding `NULL` (como hoy para texto sin modelo).
  - Si `kind != 'code'`: comportamiento actual sin cambios (bge o embedding NULL).

- `def recall(query: str, scope: str, *, kind: str = 'text', k: int, track: bool, path: str | Path) -> list[Note]:`
  - Si `kind == 'code'`: embebe `query` con `code_embedder.embed_code(query)`. Si el embedding es `None`, retorna `[]`. En el rerank fino, compara SOLO notas con `emb_model='code_embedder'`.
  - Si `kind == 'text'`: comportamiento actual, pero en el rerank fino compara SOLO notas con `emb_model != 'code_embedder'`.
  - `Note` es la clase existente en `mmorch/memory.py` con atributos `id: int` y `text: str` (y otros campos existentes).

## Requisitos

- R1: `code_embedder._load()` evalúa primero `weights.verify('code_embedder')`; si el resultado es `(False, motivo)`, almacena `False` en `_CACHE` y `available()` devuelve `False` y `embed_code()` devuelve `None`; si el resultado es `(True, _)`, usa `weights.resolve('code_embedder')` para el .npz y `weights.card('code_embedder')['vocab_path']` (relativo a `weights.ROOT`) para el vocab, y almacena `(modelo, vocab)` en `_CACHE`.
- R2: `memory.write_note(..., kind='code')` obtiene el embedding de `code_embedder.embed_code(text)`; si no es `None`, persiste `emb_model='code_embedder'` y `dim=384`; si es `None`, persiste embedding `NULL`.
- R3: `memory.recall(query, scope, ..., kind='code')` embebe la query con `code_embedder.embed_code(query)` y en el rerank fino compara SOLO notas con `emb_model='code_embedder'`; con `kind='text'` compara SOLO notas con `emb_model != 'code_embedder'` (los espacios no se mezclan).
- R4: `kind` por defecto es `'text'`; todo el comportamiento existente para `kind='text'` se conserva byte a byte. Solo se modifican `mmorch/code_embedder.py` y `mmorch/memory.py`; no se tocan tests ni otros archivos; los docstrings de módulo se conservan.

## Casos borde

- R1: `weights.verify('code_embedder')` devuelve `(False, 'sha256 distinto')` y `_CACHE` es `None`: primera llamada a `available()` devuelve `False` y deja `_CACHE` en `False`; segunda llamada a `available()` devuelve `False` sin volver a llamar a `verify`. `embed_code(CODE_A)` devuelve `None` sin inferir.
- R1: `_CACHE` ya vale `False`: `available()` devuelve `False` y `embed_code()` devuelve `None` sin llamar a `weights.verify`.
- R2: `code_embedder` no disponible (`available()` es `False`): `write_note(..., kind='code')` guarda embedding `NULL` y no falla.
- R2: `code_embedder` disponible: `write_note(..., kind='code')` guarda `emb_model='code_embedder'`, `dim=384`.
- R3: `recall(..., kind='code')` cuando no hay ninguna nota con `emb_model='code_embedder'`: el rerank fino no compara con notas de texto y el resultado es `[]` o solo notas que cumplan el filtro (vacío si no hay).
- R3: `recall(..., kind='text')` cuando hay notas de código: las notas con `emb_model='code_embedder'` se excluyen del rerank fino.
- R4: `write_note(..., kind` omitido o `kind='text'`): el comportamiento es idéntico al actual (mismo `emb_model`, misma `dim`).

## Criterios de éxito

- R1: entrada `verify('code_embedder') -> (False, 'sha256 distinto')` y `_CACHE = None` -> `available()` es `False`, `embed_code(CODE_A)` es `None`.
- R2: entrada `write_note('proj', CODE_A, kind='code', path=db)` con `code_embedder.available() == True` -> `SELECT emb_model, dim FROM semantic WHERE id = nid` devuelve `('code_embedder', 384)`.
- R3: entrada `write_note('proj', CODE_A, kind='code', path=db)`, `write_note('proj', CODE_B, kind='code', path=db)`, `write_note('proj', TEXT, path=db)` y luego `recall('def parse_lines(lines)', 'proj', kind='code', k=2, track=False, path=db)` -> el primer resultado tiene `id == a` (id de CODE_A) y ningún resultado tiene `text == TEXT`.
- R4: entrada `write_note('proj', TEXT, path=db)` -> `emb_model != 'code_embedder'`; luego `recall('bandit umbral cascada', 'proj', k=3, track=False, path=db)` -> todos los resultados tienen `id == nid` (el id de la nota de texto).

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_pesos_no_verificados_apagan_el_encoder |
| R2 | test_R2_nota_de_codigo_usa_code_embedder |
| R3 | test_R3_recall_de_codigo_solo_compara_con_codigo |
| R4 | test_R4_texto_no_toca_el_encoder |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.