"""retention — decay Ebbinghaus + Zeigarnik para la capa semantica de memory.

Modulo cognitivo #1. Determinista, cero API, cero LLM (invariante mmorch: el score
es rubric corrible, NUNCA LLM-judge). importance() es DERIVADA — no se persiste,
se computa de los inputs (access_count, last_accessed_at, open_loop). Asi nunca
queda stale.

Forgetting NUNCA pierde un hecho: solo tombstonea la nota semantica; el episodic
raw es inmutable y recall() lo recupera (FIX B). Olvidar = deja de surfacear.

ponytail: LAMBDA/K_ACC/FORGET son knobs de calibracion. Defaults Bitterbot. TUNEAR
contra datos reales antes de activar forgetting en serio (mmorch: no escalar sin
metricas). Single-user = pocas notas = conviene olvidar conservador.
"""
from __future__ import annotations

from math import exp

LAMBDA = 5e-10   # /ms. half-life decay-only ~16 dias
K_ACC = 0.2      # saturacion del factor frecuencia. n=0 -> 0.181
FORGET = 0.02    # umbral de olvido. El "rescate" aca es el episodic raw (FIX B)


def importance(now: float, last_accessed: float | None, access_count: int,
               open_loop: bool, *, lam: float = LAMBDA, k_acc: float = K_ACC,
               forget: float = FORGET) -> float:
    """Score de retencion en (0,1]. Baja con el tiempo desde el ultimo acceso,
    sube con access_count (saturante). open_loop (Zeigarnik) lo sostiene sobre el
    umbral. last_accessed=None (DB vieja) -> se trata como now (graceful, no decae)."""
    la = last_accessed if last_accessed is not None else now
    dt_ms = max(0.0, (now - la) * 1000.0)
    freq = 1.0 - exp(-k_acc * (access_count + 1))   # n=0 -> 0.181, n=5 -> 0.699
    score = freq * exp(-lam * dt_ms)
    if open_loop:                                    # Zeigarnik: resiste olvido
        score = max(score, forget * 2)
    return score


def should_forget(score: float, *, forget: float = FORGET) -> bool:
    return score < forget


if __name__ == "__main__":
    # demo/self-check (ponytail: una verificacion corrible deja el modulo terminado)
    import time
    now = time.time()
    day = 86400
    # monotonia: mas viejo => menos importance (access fijo)
    assert importance(now, now, 0, False) > importance(now, now - 30 * day, 0, False)
    # access sube
    assert importance(now, now - 10 * day, 5, False) > importance(now, now - 10 * day, 0, False)
    # zeigarnik resiste
    old = now - 365 * day
    assert should_forget(importance(now, old, 0, False))
    assert not should_forget(importance(now, old, 0, True))
    # graceful: last_accessed None no decae
    assert importance(now, None, 0, False) == importance(now, now, 0, False)
    print("retention self-check OK")
