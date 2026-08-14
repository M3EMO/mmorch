"""Health module for mmorch: dead-man's switch detection."""

import json
import time
from pathlib import Path

EXPECTATIONS = {"nightly": 26 * 3600, "server": 900, "digest": 26 * 3600}


def beat(
    component: str,
    *,
    logs_dir: str = "logs",
    now_ts: float | None = None,
    detail: str = "",
) -> None:
    """Append a heartbeat JSON line to health.jsonl (fail-open)."""
    try:
        ts = time.time() if now_ts is None else now_ts
        line = json.dumps({"component": component, "ts": ts, "detail": detail})
        path = Path(logs_dir) / "health.jsonl"
        with path.open("a", encoding="utf-8") as f:
            f.write(line + "\n")
    except Exception:
        pass


def check(
    *,
    logs_dir: str = "logs",
    now_ts: float | None = None,
    expectations: dict | None = None,
) -> dict:
    """Classify components as dead/alive/never based on heartbeat records."""
    exp = expectations if expectations is not None else EXPECTATIONS
    now = time.time() if now_ts is None else now_ts

    last_ts: dict[str, float] = {}
    path = Path(logs_dir) / "health.jsonl"
    if path.exists():
        try:
            with path.open("r", encoding="utf-8") as f:
                for line in f:
                    try:
                        rec = json.loads(line)
                        comp = rec.get("component")
                        ts = rec.get("ts")
                        if isinstance(comp, str) and isinstance(ts, (int, float)):
                            last_ts[comp] = float(ts)
                    except (json.JSONDecodeError, AttributeError):
                        continue
        except OSError:
            pass

    dead = []
    alive = []
    never = []
    for comp, limit in exp.items():
        if comp not in last_ts:
            never.append(comp)
        else:
            elapsed = now - last_ts[comp]
            if elapsed > limit:
                dead.append(
                    {
                        "component": comp,
                        "last_ts": last_ts[comp],
                        # exceso sobre el limite ("lleva X seg de mas"), no el
                        # tiempo total desde el ultimo latido
                        "overdue_s": elapsed - limit,
                    }
                )
            else:
                alive.append(comp)

    dead.sort(key=lambda d: d["component"])
    alive.sort()
    never.sort()
    return {"dead": dead, "alive": alive, "never": never}


def scrape_errors(*, logs_dir: str = "logs", max_lines: int = 50) -> dict:
    """Collect server error tail and nightly error signals (fail-open)."""
    server_err_tail: list[str] = []
    nightly_errors: dict[str, str] = {}
    idea_loop_errors: list[str] = []

    # Server error tail
    server_err_path = Path(logs_dir) / "server_forever.err"
    if server_err_path.exists():
        try:
            with server_err_path.open("r", encoding="utf-8") as f:
                lines = [l.rstrip("\n") for l in f if l.strip()]
            server_err_tail = lines[-max_lines:]
        except OSError:
            pass

    # Nightly errors from last valid record
    nightly_path = Path(logs_dir) / "nightly.jsonl"
    if nightly_path.exists():
        try:
            with nightly_path.open("r", encoding="utf-8") as f:
                last_rec = None
                for line in f:
                    try:
                        rec = json.loads(line)
                        if isinstance(rec, dict):
                            last_rec = rec
                    except json.JSONDecodeError:
                        continue
            if last_rec:
                for key, val in last_rec.items():
                    if key.endswith("_error") and isinstance(val, str):
                        nightly_errors[key] = val
                idea_loop = last_rec.get("idea_loop")
                if isinstance(idea_loop, dict):
                    errs = idea_loop.get("errors")
                    if isinstance(errs, list):
                        idea_loop_errors = [str(e) for e in errs]
        except OSError:
            pass

    return {
        "server_err_tail": server_err_tail,
        "nightly_errors": nightly_errors,
        "idea_loop_errors": idea_loop_errors,
    }


def report(*, logs_dir: str = "logs", now_ts: float | None = None) -> dict:
    """Combine check() and scrape_errors() with a healthy flag."""
    check_result = check(logs_dir=logs_dir, now_ts=now_ts)
    errors = scrape_errors(logs_dir=logs_dir)
    healthy = (
        not check_result["dead"]
        and not errors["nightly_errors"]
        and not errors["idea_loop_errors"]
    )
    return {"healthy": healthy, "check": check_result, "errors": errors}
