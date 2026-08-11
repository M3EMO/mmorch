# Rerank de recall convierte embeddings a numpy por par en loop Python

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: M
Eje: eficiencia
Evidence: mmorch/memory.py:341-358,71-77

`_cosine` por fila = 2N conversiones np.asarray + N dots chicos.

**Fix:** matriz (N×384) float32 una vez + un matmul normalizado contra qvec.

## Comments
`_cosine_batch(qvec, embs)`: 1 matriz (N x 384) float32 + norms por fila + 1 matmul contra
qvec normalizado, mismo resultado que `_cosine` fila a fila (incluye el caso vector-cero).
`recall()` arma `valid` (solo filas con embedding) y llama `_cosine_batch` una vez en vez
de N `_cosine` dentro del loop de rerank. `_is_dup` (consolidate, dedup pairwise) se deja
con `_cosine` sin tocar — no es el loop de rerank que pedía el ticket.
Test de equivalencia: `tests/test_cosine_batch.py` (random 384d, vector-cero, query-cero,
lista vacía) — todo dentro de 1e-5 del resultado serial.
