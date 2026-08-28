"""canary — set FIJO de tareas con respuesta verificable deterministicamente (W5.3).

Por que: los providers rotan versiones de modelo sin aviso (research 08: el "temporal
drift" de Arize; glm-4.6 midio 34% error ventaneado sin que nada lo detectara). Este
modulo corre ~20 tareas congeladas contra los modelos activos, verifica cada respuesta
con un checker DETERMINISTA (checkers.py — nunca un LLM juez) y compara el pass-rate
por modelo contra un baseline guardado: una caida silenciosa del provider se vuelve
señal visible en un comando (`mmorch canary`). Barato: modelos externos, cero cupo.

El set de tareas viaja con el CODIGO (canary_tasks.jsonl versionado junto al paquete,
igual que prompts/); el baseline es ESTADO y vive en logs_dir() (paths.py).
"""
from __future__ import annotations

import json
import re
import time
from pathlib import Path
from typing import Any

from . import providers
from .checkers import check
from .feedback import record_outcome as _record_outcome
from .paths import logs_dir

# el set congelado viaja con el codigo (no es estado -> no pasa por paths.py)
_TASKS_FILE = Path(__file__).with_name("canary_tasks.jsonl")

# caida de pass-rate vs baseline que se reporta como drift. 0.1 = 2 tareas de 20:
# una sola tarea caida puede ser ruido de sampling; dos ya es señal de provider.
DRIFT_DROP = 0.1

_NUM_RE = re.compile(r"[-+]?\d+(?:\.\d+)?")


def load_tasks(path: Path | None = None) -> list[dict]:
    """Carga el set canario (jsonl versionado). Linea corrupta = error duro:
    el set es codigo congelado, no un log tolerante — un typo debe romper YA."""
    p = path or _TASKS_FILE
    tasks = []
    for ln in p.read_text(encoding="utf-8").splitlines():
        if ln.strip():
            tasks.append(json.loads(ln))
    return tasks


def _extract_answer(text: str, checker: str) -> str:
    """Normaliza la respuesta del modelo al valor que el checker compara. Los prompts
    piden 'SOLO el valor', pero los modelos igual decoran (comillas, punto final,
    separadores de miles) — extraer aca evita falsos rojos que no son drift real."""
    t = (text or "").strip()
    if checker == "numeric_close":
        nums = _NUM_RE.findall(t.replace(",", ""))
        return nums[-1] if nums else t
    line = next((s.strip() for s in t.splitlines() if s.strip()), "")
    # strip combinado (comillas/backticks/puntos/espacios en cualquier orden):
    # '"XIV".' debe quedar 'XIV', no 'XIV"'
    return line.strip("\"'` .")


def _fill(ctx: dict, answer: str) -> dict:
    """Sustituye el placeholder "{answer}" del ctx por la respuesta extraida
    (misma convencion que rubric_loop con {attempt})."""
    return {k: (answer if v == "{answer}" else v) for k, v in ctx.items()}


def run_canary(models: list[str] | None = None, tasks: list[dict] | None = None,
               record: bool = True) -> dict[str, dict]:
    """Corre el set contra cada modelo. Devuelve {model: resultado} donde resultado es
    {passed, total, pass_rate, model_version, failures} o {"skipped": razon} si falta
    la API key (degradacion clara, nunca crash). record=True loggea un outcome por
    modelo (pattern='canary', con model_version del provider) — asi el historial de
    feedback queda etiquetado con la version exacta que produjo cada medicion."""
    from .config import DEFAULT_GENERATOR, DEFAULT_ROUTER, DEFAULT_VERIFIER
    if models is None:
        # los "activos" por default: el trio que usa el flujo diario (gen/verify/route)
        models = list(dict.fromkeys([DEFAULT_GENERATOR, DEFAULT_VERIFIER, DEFAULT_ROUTER]))
    if tasks is None:
        tasks = load_tasks()
    out: dict[str, dict] = {}
    for m in models:
        passed, failures, version, skipped = 0, [], "", ""
        for t in tasks:
            try:
                res = providers.call(m, t["prompt"], pattern="canary", node=t["id"],
                                     temperature=0.0, max_tokens=128)
            except providers.MissingKeyError as e:
                skipped = str(e)   # sin key -> se saltea el MODELO entero, con mensaje
                break
            except Exception as e:
                # fallo de API en UNA tarea (timeout/5xx post-retry): cuenta como
                # fallida — un provider que no responde ES drift operacional
                failures.append({"id": t["id"], "detail": f"api_error: {str(e)[:120]}"})
                continue
            version = res.model_version or version
            ans = _extract_answer(res.text, t["checker"])
            try:
                r = check(t["checker"], **_fill(t["ctx"], ans))
                ok, detail = r.passed, r.detail
            except Exception as e:
                ok, detail = False, f"checker_error: {str(e)[:120]}"
            if ok:
                passed += 1
            else:
                failures.append({"id": t["id"], "detail": str(detail)[:160]})
        if skipped:
            out[m] = {"skipped": skipped}
            continue
        n = len(tasks)
        rate = round(passed / n, 3) if n else 0.0
        out[m] = {"passed": passed, "total": n, "pass_rate": rate,
                  "model_version": version, "failures": failures[:5]}
        if record:
            _record_outcome(m, rate, pattern="canary", source="checker",
                            model_version=version)
    return out


def baseline_path() -> Path:
    return logs_dir() / "canary_baseline.json"


def compare_baseline(results: dict[str, dict], *, update: bool = False,
                     path: Path | None = None,
                     drift_drop: float = DRIFT_DROP) -> dict[str, Any]:
    """Anota cada resultado medido con su baseline y flaggea drift (caida de
    pass-rate > drift_drop). update=True persiste los pass-rates medidos como nuevo
    baseline (merge por modelo: no borra modelos que hoy no se corrieron)."""
    p = path or baseline_path()
    try:
        base: dict = json.loads(p.read_text(encoding="utf-8")) if p.exists() else {}
    except Exception:
        base = {}   # baseline corrupto = como si no hubiera; el proximo update lo repara
    drift: list[str] = []
    for m, r in results.items():
        if "pass_rate" not in r:
            continue
        prev = base.get(m, {}).get("pass_rate")
        r["baseline"] = prev
        r["drift"] = prev is not None and r["pass_rate"] < prev - drift_drop
        if r["drift"]:
            drift.append(m)
        if update:
            base[m] = {"pass_rate": r["pass_rate"],
                       "model_version": r.get("model_version", ""), "ts": time.time()}
    if update:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(json.dumps(base, ensure_ascii=False, indent=2), encoding="utf-8")
    return {"models": results, "drift": drift, "baseline_updated": bool(update)}
