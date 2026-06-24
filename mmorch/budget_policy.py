"""budget_policy — scoped budget policies (graft G5 from paperclip).

Policy-as-data on top of the existing global BudgetKeeper: a list of
{scope, limit_usd, warn_pct} where scope is a key into the spend snapshot
("global" = this month; "family:deepseek"/"family:google" = lifetime, the data
mmorch actually tracks). soft incident at warn_pct, hard at >= limit. A hard
incident blocks NEW work at job creation. Persisted (portable via G4-style file).

ponytail: pure evaluate() (unit-tested) + a json file + a thin gate in server.py.
Per-project scopes are a follow-up (needs per-project cost attribution, not tracked yet).
"""
from __future__ import annotations

import json
import os
from pathlib import Path

_PATH = Path(os.getenv("MMORCH_BUDGET_POLICIES")
             or (Path(__file__).resolve().parent.parent / "budget_policies.json"))


def load() -> list:
    try:
        return json.loads(_PATH.read_text(encoding="utf-8")) if _PATH.exists() else []
    except Exception:
        return []


def save(policies: list) -> None:
    _PATH.write_text(json.dumps(list(policies), indent=1), encoding="utf-8")


def snapshot() -> dict:
    """Spend by scope, from the data mmorch tracks: global(month) + family(lifetime)."""
    from .budget import monthly_spend
    from .metrics import summary
    snap = {"global": round(float(monthly_spend()), 6)}
    for fam, c in (summary().get("cost_by_family") or {}).items():
        snap[f"family:{fam}"] = round(float(c), 6)
    return snap


def evaluate(policies: list, snap: dict) -> list:
    """Incidents for crossed thresholds. soft = warn reached, hard = limit reached."""
    out = []
    for p in policies or []:
        scope = p.get("scope")
        limit = float(p.get("limit_usd", 0) or 0)
        warn = float(p.get("warn_pct", 80) or 80)
        if limit <= 0:
            continue
        spent = float(snap.get(scope, 0.0))
        pct = 100.0 * spent / limit
        if spent >= limit:
            level = "hard"
        elif pct >= warn:
            level = "soft"
        else:
            continue
        out.append({"scope": scope, "level": level, "spent": round(spent, 6),
                    "limit": limit, "pct": round(pct, 1)})
    return out


def blocking_incident(snap: dict | None = None, policies: list | None = None) -> dict | None:
    snap = snapshot() if snap is None else snap
    policies = load() if policies is None else policies
    for inc in evaluate(policies, snap):
        if inc["level"] == "hard":
            return inc
    return None


if __name__ == "__main__":
    P = [{"scope": "global", "limit_usd": 10, "warn_pct": 80},
         {"scope": "family:deepseek", "limit_usd": 5, "warn_pct": 90}]
    assert evaluate(P, {"global": 5}) == [], "under warn -> no incident"
    soft = evaluate(P, {"global": 8})
    assert soft and soft[0]["level"] == "soft", soft
    hard = evaluate(P, {"global": 10, "family:deepseek": 4})
    assert any(i["level"] == "hard" and i["scope"] == "global" for i in hard), hard
    assert blocking_incident({"global": 12}, P)["scope"] == "global"
    assert blocking_incident({"global": 1}, P) is None
    assert evaluate([{"scope": "global", "limit_usd": 0}], {"global": 99}) == [], "limit<=0 ignored"
    print("budget_policy OK")
