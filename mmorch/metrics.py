"""Observability — append-only JSONL metric log (§11 backbone).

Without these metrics you cannot route, nor falsify the break-even D_total > U_max.
Every node call logs one record. The log is the input to the parametric cost sheet
and to the A/B/C ablation (§18.4).

Logged per call: timestamp, phase, pattern, node label, model, family,
in/out tokens, cost_usd, latency_s, plus any extra fields.
"""
from __future__ import annotations

import json
import os
import threading
import time
from pathlib import Path

_LOCK = threading.Lock()

# logs/ sits next to this package, under ~/.claude/orchestration/
_LOG_DIR = Path(__file__).resolve().parent.parent / "logs"
_LOG_PATH = _LOG_DIR / "metrics.jsonl"


def log_path() -> Path:
    return _LOG_PATH


def log_event(
    *,
    pattern: str,
    node: str,
    model: str,
    family: str,
    in_tokens: int,
    out_tokens: int,
    cost_usd: float,
    latency_s: float,
    phase: str = "",
    **extra,
) -> None:
    record = {
        "ts": time.time(),
        "iso": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime()),
        "phase": phase,
        "pattern": pattern,
        "node": node,
        "model": model,
        "family": family,
        "in_tokens": in_tokens,
        "out_tokens": out_tokens,
        "cost_usd": round(cost_usd, 8),
        "latency_s": round(latency_s, 4),
    }
    if extra:
        record["extra"] = extra
    line = json.dumps(record, ensure_ascii=False)
    with _LOCK:
        _LOG_DIR.mkdir(parents=True, exist_ok=True)
        with open(_LOG_PATH, "a", encoding="utf-8") as fh:
            fh.write(line + "\n")


def read_events() -> list[dict]:
    if not _LOG_PATH.exists():
        return []
    out = []
    with open(_LOG_PATH, encoding="utf-8") as fh:
        for ln in fh:
            ln = ln.strip()
            if ln:
                out.append(json.loads(ln))
    return out


def summary() -> dict:
    """Aggregate the log: total cost, tokens, calls per family/model."""
    events = read_events()
    total_cost = sum(e["cost_usd"] for e in events)
    by_family: dict[str, float] = {}
    by_model: dict[str, int] = {}
    for e in events:
        by_family[e["family"]] = by_family.get(e["family"], 0.0) + e["cost_usd"]
        by_model[e["model"]] = by_model.get(e["model"], 0) + 1
    return {
        "calls": len(events),
        "total_cost_usd": round(total_cost, 6),
        "cost_by_family": {k: round(v, 6) for k, v in by_family.items()},
        "calls_by_model": by_model,
    }
