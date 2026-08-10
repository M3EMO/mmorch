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


# ---------------------------------------------------------------------------
# Ranking de recall (patron leido del codigo de grok-build/xai-grok-memory,
# 2026-07: score = relevancia x decay_temporal x boost_de_frecuencia + rerank
# MMR por diversidad). Antes recall() rankeaba por cosine PURO — importance()
# existia pero solo para OLVIDAR, nunca para rankear: una nota tocada 20 veces
# ayer y una intacta de hace 6 meses empataban si el cosine empataba.
# ---------------------------------------------------------------------------
def rank_score(relevance: float, now: float, last_accessed: float | None,
               access_count: int, open_loop: bool, *, lam: float = LAMBDA) -> float:
    """Score final de recall: relevancia (cosine) modulada por retencion.
    decay = exp(-lam*dt) desde el ultimo acceso; open_loop (Zeigarnik) le pone piso
    0.5 al decay (analogo a la exencion 'evergreen' de grok — vivo, no decae a 0).
    boost = 1 + ln(1+access)*0.05 (grok: 0 accesos -> 1.0, 10 -> ~1.12; modesto a
    proposito, la relevancia manda, la frecuencia desempata)."""
    from math import log1p
    la = last_accessed if last_accessed is not None else now
    decay = exp(-lam * max(0.0, (now - la) * 1000.0))
    if open_loop:
        decay = max(decay, 0.5)
    return relevance * decay * (1.0 + log1p(access_count) * 0.05)


def mmr_rerank(items: list, scores: list[float], k: int, *, lam: float = 0.7) -> list:
    """Maximal Marginal Relevance (grok-build mmr.rs): seleccion greedy que
    balancea relevancia con diversidad — sin esto, 5 notas casi-duplicadas del
    mismo tema copan el top-k. MMR(d) = lam*rel(d) - (1-lam)*max_sim(d, elegidos).
    Similitud = Jaccard sobre tokens (SIN embeddings, O(n*k) con n chico).
    `items` = objetos con .text; `scores` alineado por indice. Devuelve <= k items."""
    if len(items) <= 1 or k <= 0:
        return items[:k]

    def toks(t: str) -> set:
        return {w for w in "".join(c if c.isalnum() or c == "_" else " "
                                   for c in t.lower()).split() if w}

    tok = [toks(it.text) for it in items]

    def jac(a: set, b: set) -> float:
        if not a or not b:
            return 1.0 if (not a and not b) else 0.0
        inter = len(a & b)
        return inter / (len(a) + len(b) - inter)

    chosen: list[int] = []
    rest = list(range(len(items)))
    while rest and len(chosen) < k:
        best_i, best_v = rest[0], float("-inf")
        for i in rest:
            penalty = max((jac(tok[i], tok[j]) for j in chosen), default=0.0)
            v = lam * scores[i] - (1.0 - lam) * penalty
            if v > best_v:
                best_i, best_v = i, v
        chosen.append(best_i)
        rest.remove(best_i)
    return [items[i] for i in chosen]


# --------------------------------------------------------------------------- #
# derive_tier (graft Estudio app/src/engine/progress/derive-tier.ts, 2026-08-07):
# maquina de estados de mastery para RESURFACING — decide que item re-mostrar.
# Reglas EN ORDEN, la primera que aplica gana. Ventanas en multiplos de 7 dias.
# La guardia de solido evita "score reciente bueno pero fallas viejas sin
# resolver". Uso en mmorch: items del vault / notas / conceptos con historial
# de outcomes (ok, first_try, last_seen_ts).
# --------------------------------------------------------------------------- #
_DAY = 86400.0
FAIL_WINDOW = 7 * _DAY      # fallo en la ultima semana -> repasar
STALE_WINDOW = 56 * _DAY    # sin verse en 8 semanas -> repasar
FRESH_WINDOW = 21 * _DAY    # solido requiere visto en las ultimas 3 semanas
SOLIDO_FIRST_TRY_RATIO = 0.8


def derive_tier(records: list[dict], now: float) -> str:
    """'nuevo' | 'repasar' | 'solido' | 'en-progreso'.

    records: [{"ok": bool, "first_try": bool, "last_seen": ts}] — un registro por
    item/pregunta que cubre el concepto, con al menos un intento."""
    rel = [r for r in records if r.get("last_seen") is not None]
    if not rel:
        return "nuevo"
    if any(not r.get("ok") and now - r["last_seen"] <= FAIL_WINDOW for r in rel):
        return "repasar"
    if all(now - r["last_seen"] > STALE_WINDOW for r in rel):
        return "repasar"
    fresh_first = [r for r in rel
                   if r.get("first_try") and now - r["last_seen"] <= FRESH_WINDOW]
    ratio = sum(1 for r in rel if r.get("first_try")) / len(rel)
    if len(fresh_first) >= 2 and ratio >= SOLIDO_FIRST_TRY_RATIO:
        return "solido"
    return "en-progreso"


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

    # rank_score: mismo cosine, nota fresca+tocada gana a nota vieja intacta
    assert rank_score(0.8, now, now, 10, False) > rank_score(0.8, now, now - 60 * day, 0, False)
    # relevancia manda: cosine mucho mayor gana aunque este mas vieja (dentro de lo razonable)
    assert rank_score(0.9, now, now - 5 * day, 0, False) > rank_score(0.5, now, now, 3, False)
    # open_loop pone piso al decay (nota vieja con loop abierto no muere en el ranking)
    assert rank_score(0.8, now, now - 90 * day, 0, True) >= 0.8 * 0.5
    # boost modesto: nunca infla mas de ~30% con 100 accesos
    assert rank_score(0.5, now, now, 100, False) < 0.5 * 1.3

    # mmr_rerank: con 3 notas casi-identicas y 1 distinta, el top-2 NO son 2 duplicadas
    class _N:
        def __init__(self, t):
            self.text = t
    dups = [_N("el gato come pescado fresco"), _N("el gato come pescado muy fresco"),
            _N("el gato come pescado fresco hoy"), _N("configurar timeout del servidor http")]
    got = mmr_rerank(dups, [0.9, 0.89, 0.88, 0.6], 2)
    assert got[0].text.startswith("el gato") and got[1].text.startswith("configurar"), \
        [n.text for n in got]
    # k >= n devuelve todo; k=0 devuelve nada
    assert len(mmr_rerank(dups, [1, 1, 1, 1], 10)) == 4
    assert mmr_rerank(dups, [1, 1, 1, 1], 0) == []

    # derive_tier: las 4 reglas en orden + la guardia de solido
    t = now
    assert derive_tier([], t) == "nuevo"
    assert derive_tier([{"ok": False, "first_try": False, "last_seen": t - 2 * _DAY}],
                       t) == "repasar", "fallo reciente"
    assert derive_tier([{"ok": True, "first_try": True, "last_seen": t - 60 * _DAY}],
                       t) == "repasar", "todo stale"
    solido = [{"ok": True, "first_try": True, "last_seen": t - 5 * _DAY},
              {"ok": True, "first_try": True, "last_seen": t - 10 * _DAY},
              {"ok": True, "first_try": True, "last_seen": t - 30 * _DAY}]
    assert derive_tier(solido, t) == "solido"
    # GUARDIA: 2 first-try frescos pero ratio global 0.5 < 0.8 -> en-progreso
    mixed = solido[:2] + [{"ok": True, "first_try": False, "last_seen": t - 30 * _DAY},
                          {"ok": True, "first_try": False, "last_seen": t - 40 * _DAY}]
    assert derive_tier(mixed, t) == "en-progreso", "guardia de fallas viejas"
    print("retention self-check OK (rank+mmr+derive_tier)")
