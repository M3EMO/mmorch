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
import logging
import os
from pathlib import Path

from .iohelpers import atomic_write_json

_log = logging.getLogger(__name__)

from .paths import data_dir

_PATH = Path(os.getenv("MMORCH_BUDGET_POLICIES")
             or (data_dir() / "budget_policies.json"))


class PolicyLoadError(Exception):
    """budget_policies.json existe pero no parsea (distinto de 'no hay politicas')."""


def load(*, strict: bool = False) -> list:
    """[] es un default legitimo solo si el archivo NO existe. Si existe pero esta
    corrupto, loggeamos fuerte; con strict=True ademas propagamos, para que
    blocking_incident() pueda fallar CERRADO (bloquear gasto) en vez de fallar abierto
    en silencio — un JSON truncado no debe poder desactivar los hard-stops de gasto."""
    if not _PATH.exists():
        return []
    try:
        return json.loads(_PATH.read_text(encoding="utf-8"))
    except Exception:
        _log.error("budget_policies.json corrupto en %s", _PATH)
        if strict:
            raise PolicyLoadError(str(_PATH)) from None
        return []


def save(policies: list) -> None:
    atomic_write_json(_PATH, list(policies), indent=1)


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
    if policies is None:
        try:
            policies = load(strict=True)
        except PolicyLoadError:
            # conservador: politicas ilegibles = no se puede confirmar que el gasto este
            # dentro de limite -> bloquear trabajo nuevo hasta que se repare el archivo,
            # en vez de dejar pasar todo sin señal (el bug que este fix cierra).
            return {"scope": "*", "level": "hard", "spent": 0.0, "limit": 0.0, "pct": 100.0,
                    "reason": f"budget_policies.json corrupto en {_PATH}"}
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
    assert blocking_incident({"global": 12}, P)["scope"] == "global"  # type: ignore[index]
    assert blocking_incident({"global": 1}, P) is None
    assert evaluate([{"scope": "global", "limit_usd": 0}], {"global": 99}) == [], "limit<=0 ignored"
    print("budget_policy OK")
