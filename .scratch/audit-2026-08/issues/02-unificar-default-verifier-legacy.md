# Defaults de verificador en cache.py/ensemble.py usan gemini-2.5-flash legacy (más caro)

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: S
Eje: eficiencia
Evidence: mmorch/cache.py:44,51 · mmorch/ensemble.py:51,195 · mmorch/config.py:161

`cache.memoized_verify` default `verifier_model="gemini-2.5-flash"` (out $2.50/M) y
`ensemble_verify` default `["gemini-2.5-flash","gemini-2.5-flash-lite"]`, mientras
`DEFAULT_VERIFIER="gemini-3.1-flash-lite"` (out $1.50/M) y `pair_verify` ya migró.
Doble costo: out-price +67% + fragmentación del memo-cache (la key incluye verifier_model).

**Fix:** importar y usar `DEFAULT_VERIFIER` en ambos defaults.

## Comments
cache.py:14,66 y ensemble.py:13,52 ya importan y usan `DEFAULT_VERIFIER` (heredado del
agente anterior). Verificado: sin `gemini-2.5-flash` residual en ninguno de los dos defaults.
