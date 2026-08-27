"""Health module for mmorch: dead-man's switch detection."""

import json
import logging
import time
from pathlib import Path

logger = logging.getLogger(__name__)

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
    except OSError as e:
        logger.warning("beat(): cannot write health.jsonl: %s", e)
    except Exception as e:
        # Fail-open: heartbeat must never break the main path even if
        # detail contains non-serializable types or ts is invalid.
        # These are caller bugs, not health infrastructure failures.
        logger.debug("beat(): non-OSError ignored (fail-open): %s", e)


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
        except OSError as e:
            logger.warning("check(): cannot read health.jsonl: %s", e)

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
            raw = server_err_path.read_bytes()
            # PowerShell redirige stderr en UTF-16 con BOM (medido: 0xff 0xfe)
            enc = "utf-16" if raw[:2] in (b"\xff\xfe", b"\xfe\xff") else "utf-8"
            lines = [ln.rstrip("\n") for ln in
                     raw.decode(enc, errors="replace").splitlines() if ln.strip()]
            server_err_tail = lines[-max_lines:]
        except OSError as e:
            logger.warning("scrape_errors(): cannot read server_forever.err: %s", e)

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
        except OSError as e:
            logger.warning("scrape_errors(): cannot read nightly.jsonl: %s", e)

    return {
        "server_err_tail": server_err_tail,
        "nightly_errors": nightly_errors,
        "idea_loop_errors": idea_loop_errors,
    }


def _recent_silent_errors(logs_dir: str, *, hours: float = 48.0) -> list[dict]:
    """Cola de logs/silent_errors.jsonl (canal generico que llena nightly.py
    con findings_from_record de CADA subsistema) recortada a lo reciente.
    No reemplaza el log propio de cada modulo — es el INDICE de lectura, no
    la fuente: cada subsistema sigue siendo dueño de su propio detalle."""
    import json as _json
    path = Path(logs_dir) / "silent_errors.jsonl"
    try:
        lines = path.read_text(encoding="utf-8").strip().splitlines()
    except OSError as e:
        logger.warning("_recent_silent_errors(): cannot read silent_errors.jsonl: %s", e)
        return []
    from datetime import date, timedelta
    # timedelta(hours=hours), NO hours/24: aquel daba 2 HORAS que la
    # aritmetica de date trunca a 0 dias -> corte siempre HOY -> toda entrada
    # de ayer quedaba afuera (bug cazado por evolve_red.jsonl en su estreno:
    # los 6 sandboxes de la noche murieron por este test, no por sus parches)
    corte = (date.today() - timedelta(hours=hours)).isoformat()
    out = []
    # "rec", no "e": el except de arriba ya uso "e" y reciclar el nombre
    # fuera del except es error de mypy (CPython lo borra al salir del bloque)
    for ln in lines[-200:]:
        try:
            rec = _json.loads(ln)
        except _json.JSONDecodeError:
            continue
        if rec.get("fecha", "") >= corte:
            out.append(rec)
    return out


def report(*, logs_dir: str = "logs", now_ts: float | None = None) -> dict:
    """Combine check() y scrape_errors() con un flag healthy, mas un vistazo
    a silent_errors.jsonl: una sola llamada responde '¿vivo?' Y '¿algo roto
    reciente en cualquier subsistema?' sin fusionar quien es dueño de que."""
    check_result = check(logs_dir=logs_dir, now_ts=now_ts)
    errors = scrape_errors(logs_dir=logs_dir)
    recientes = _recent_silent_errors(logs_dir)
    healthy = (
        not check_result["dead"]
        and not errors["nightly_errors"]
        and not errors["idea_loop_errors"]
    )
    return {"healthy": healthy, "check": check_result, "errors": errors,
            "silent_errors_48h": len(recientes),
            "silent_errors_sample": recientes[:5]}


def check_projects(projects: dict, *, run_fn=None, timeout: float = 600.0) -> dict:
    """Suite de cada proyecto del registry que tenga tests/ — suite roja = señal
    de bug que nadie vio (la otra mitad del dead-man's switch: no solo procesos
    muertos, tambien verdad de ejecucion por proyecto).

    run_fn(path) -> bool (True = suite verde). Default: pytest -q -x con el
    python del PROPIO proyecto si tiene .venv (correr mmorch's venv sobre otro
    repo daria falsas alarmas de imports), fail-soft por proyecto."""
    import subprocess
    import sys as _sys
    from pathlib import Path as _P

    def _default_run(path: str) -> bool:
        py = _P(path) / ".venv" / "Scripts" / "python.exe"
        exe = str(py) if py.exists() else _sys.executable
        import os
        import tempfile
        bt = tempfile.mkdtemp(prefix="mmorch_health_")
        # SUITE_BUDGET_S: cuanto tiempo de TESTS le presta este chequeo al
        # proyecto (el resto del timeout se lo come la coleccion). El proyecto
        # decide que recortar — si su conftest no lo implementa, lo ignora y
        # nada cambia. Medido 2026-08-24: "Portfolio financiero" tarda 20 min,
        # el chequeo daba 10, y el timeout tapaba 2 tests rojos reales durante
        # 9 noches. Un veredicto parcial y explicito vale mas que ninguno.
        env = {**os.environ, "SUITE_BUDGET_S": str(round(timeout * 0.75))}
        # --basetemp: el pytest-current global de Windows queda con permisos
        # rotos (root cause documentada), y sin esto la limpieza final tira
        # PermissionError en CADA proyecto ajeno que se chequea aca
        r = subprocess.run([exe, "-m", "pytest", "-q", "-x",
                            f"--basetemp={bt}"], cwd=path,
                           capture_output=True, timeout=timeout, env=env)
        return r.returncode == 0

    run = run_fn or _default_run
    ok: list[str] = []
    failing: list[str] = []
    skipped: list[str] = []
    errors: list[str] = []
    for name in sorted(projects):
        path = projects[name]
        try:
            if not (_P(path) / "tests").is_dir():
                skipped.append(name)
                continue
            (ok if run(path) else failing).append(name)
        except Exception as e:  # timeout/venv roto: señal, no crash del health
            errors.append(f"{name}: {type(e).__name__}: {str(e)[:120]}")
    return {"ok": ok, "failing": failing, "sin_tests": skipped, "errors": errors}