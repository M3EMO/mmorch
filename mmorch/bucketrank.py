"""bucket_rank — graduar/ordenar un set GRANDE en tiers (triage por calidad, rankear
N>>10). NO es pairwise (eso es tournament, para pocos por gusto): aca cada item se
clasifica independiente en un tier por un modelo BARATO en paralelo. O(n) llamadas,
no O(n^2). Rol 'classification' (gemini-flash-lite por default).

No exige cross-family: no verifica el output de un generador, GRADUA items dados
(como route/classify). Si se quiere robustez, verificar los tiers borde es follow-up.
Alineacion item<->tier preservada aunque una llamada falle (cae al tier mas bajo).
"""
from __future__ import annotations

import re
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field

from .config import DEFAULT_ROUTER
from .providers import call

_TIER_RE = re.compile(r"TIER\s*[:=]\s*([A-Za-z0-9]+)", re.I)
_DEFAULT_TIERS = ["S", "A", "B", "C", "D"]


@dataclass
class BucketRankResult:
    by_tier: dict[str, list[str]] = field(default_factory=dict)
    graded: list[dict] = field(default_factory=list)   # [{item, tier}]
    cost_usd: float = 0.0
    n_failed: int = 0


def _grade_one(item: str, rubric: str, tiers: list[str], model: str, phase: str):
    sys_msg = (
        f"Sos un clasificador. Graduá el item segun el criterio en UNO de estos tiers: "
        f"{', '.join(tiers)} (de mejor a peor). Al final, en una linea aparte, escribi "
        f"exactamente: TIER: <uno de {', '.join(tiers)}>.")
    user = f"CRITERIO:\n{rubric}\n\nITEM:\n{item}"
    res = call(model, [{"role": "system", "content": sys_msg},
                       {"role": "user", "content": user}],
               pattern="bucket_rank", node="grader", phase=phase, temperature=0.0)
    return _extract_tier(res.text, tiers), res.cost_usd


def _extract_tier(text: str, tiers: list[str]) -> str:
    m = _TIER_RE.search(text or "")
    if m:
        cand = m.group(1).upper()
        for t in tiers:
            if t.upper() == cand:
                return t
    return tiers[-1]  # sin senal clara -> tier mas bajo (conservador)


def bucket_rank(
    items: list[str],
    *,
    rubric: str,
    tiers: list[str] | None = None,
    grader_model: str = DEFAULT_ROUTER,
    max_workers: int = 8,
    phase: str = "",
) -> BucketRankResult:
    """Clasifica cada item en un tier (paralelo, barato). Devuelve agrupado por tier."""
    tiers = tiers or _DEFAULT_TIERS
    out = BucketRankResult(by_tier={t: [] for t in tiers})
    if not items:
        return out
    results: list[tuple[str, float] | None] = [None] * len(items)

    def _job(idx_item):
        i, it = idx_item
        try:
            return i, _grade_one(it, rubric, tiers, grader_model, phase)
        except Exception:
            return i, None

    with ThreadPoolExecutor(max_workers=max_workers) as ex:
        futs = [ex.submit(_job, (i, it)) for i, it in enumerate(items)]
        for f in as_completed(futs):
            i, r = f.result()
            results[i] = r

    for it, r in zip(items, results, strict=False):
        if r is None:
            out.n_failed += 1
            tier = tiers[-1]  # fallo -> tier mas bajo, item NO se pierde
        else:
            tier, cost = r
            out.cost_usd += cost
        out.by_tier[tier].append(it)
        out.graded.append({"item": it, "tier": tier})
    out.cost_usd = round(out.cost_usd, 6)
    return out
