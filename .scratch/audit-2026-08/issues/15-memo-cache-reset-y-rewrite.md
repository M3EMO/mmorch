# Memo cache: reset silencioso + rewrite completo por put, lock solo-thread, sin singleton

Type: task
Status: open
Severity: NICE-TO-HAVE
Effort: S
Eje: robustez + eficiencia (R7 + E5 mergeados)
Evidence: mmorch/cache.py:13,25-29,34-38,50

logs/memo.json corrupto → `{}` silencioso (se re-paga API por verifies ya cacheados);
cada `put` reserializa el archivo entero (~100 KB, sin poda) sin atomicidad; `_LOCK` es
threading (no cubre multi-proceso); `Memo()` fresco por `memoized_verify(memo=None)`
relee el archivo entero por llamada.

**Fix:** singleton módulo-level + write atómico; backend con lookup O(1) y escritura
incremental (sqlite/shelve, o JSONL cargado a dict) si crece.

## Comments
Ya completo (heredado): log de corrupción (no reset silencioso), write atómico
(tmp + os.replace), singleton módulo-level `default_memo()` con el guard de falsy-Memo
preservado en `memoized_verify`. Verificado con `tests/test_cache_cost.py`. Cierro.
