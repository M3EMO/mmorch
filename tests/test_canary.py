"""W5.3 — canary set: drift silencioso de provider se vuelve señal en un comando.

providers.call SIEMPRE mockeado (cero API, cero $): lo que se testea es el circuito
tareas congeladas -> checker determinista -> pass-rate -> baseline -> drift, y la
degradacion clara cuando no hay API keys.
"""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import mmorch.canary as C
import mmorch.providers as PR
from mmorch.checkers import available
from mmorch.providers import CallResult


def _fake_result(text, version="deepseek-v4-flash-20260801"):
    return CallResult("deepseek-chat", "deepseek", text, 1, 1, 0.0, 0.0,
                      model_version=version)


# respuestas correctas de las tareas regex (las numericas salen del ctx del set)
_REGEX_ANS = {
    "iso-navidad-2024": "2024-12-25", "hex-rojo": "#FF0000", "bin-10": "1010",
    "romano-14": "XIV", "capital-francia": "París", "plural-mouse": "mice",
}


def _answer_for(task) -> str:
    if task["checker"] == "regex_format":
        return _REGEX_ANS[task["id"]]
    return str(task["ctx"]["b"])


def test_set_congelado_valido():
    tasks = C.load_tasks()
    assert len(tasks) == 20
    ids = [t["id"] for t in tasks]
    assert len(set(ids)) == 20, "ids duplicados"
    regs = set(available())
    for t in tasks:
        assert t["checker"] in regs, f"{t['id']}: checker no registrado"
        # cada tarea debe inyectar la respuesta del modelo en el checker
        assert "{answer}" in t["ctx"].values(), t["id"]


def test_extract_answer_tolera_decoracion():
    assert C._extract_answer("La respuesta es 184,756.", "numeric_close") == "184756"
    assert C._extract_answer("**42**", "numeric_close") == "42"
    assert C._extract_answer('"XIV".', "regex_format") == "XIV"
    assert C._extract_answer("París\n(la capital)", "regex_format") == "París"


def test_run_canary_todo_verde(monkeypatch):
    tasks = C.load_tasks()
    by_id = {t["id"]: t for t in tasks}

    def fake_call(model, messages, **k):
        t = by_id[k["node"]]
        ans = _answer_for(t)
        # decoracion tipica de modelo real: prosa en numericas, comillas+punto en texto
        text = f"La respuesta es {ans}." if t["checker"] == "numeric_close" else f'"{ans}".'
        return _fake_result(text)
    monkeypatch.setattr(PR, "call", fake_call)
    recorded = []
    monkeypatch.setattr(C, "_record_outcome", lambda *a, **k: recorded.append((a, k)))

    res = C.run_canary(models=["deepseek-chat"], tasks=tasks)
    r = res["deepseek-chat"]
    assert r["pass_rate"] == 1.0 and r["passed"] == r["total"] == 20
    # model_version viaja del provider al reporte Y al outcome (invalida priors al rotar)
    assert r["model_version"] == "deepseek-v4-flash-20260801"
    assert recorded and recorded[0][1]["model_version"] == "deepseek-v4-flash-20260801"
    assert recorded[0][1]["pattern"] == "canary"


def test_run_canary_cuenta_fallos(monkeypatch):
    tasks = C.load_tasks()[:4]
    monkeypatch.setattr(PR, "call", lambda m, msgs, **k: _fake_result("999999999"))
    res = C.run_canary(models=["deepseek-chat"], tasks=tasks, record=False)
    r = res["deepseek-chat"]
    assert r["passed"] == 0 and r["pass_rate"] == 0.0 and len(r["failures"]) == 4


def test_degrada_sin_api_key(monkeypatch):
    def no_key(*a, **k):
        raise PR.MissingKeyError("env var DEEPSEEK_API_KEY not set")
    monkeypatch.setattr(PR, "call", no_key)
    res = C.run_canary(models=["deepseek-chat"], record=False)
    # sin key = modelo salteado con razon clara, jamas crash ni pass-rate falso
    assert "skipped" in res["deepseek-chat"]
    assert "DEEPSEEK_API_KEY" in res["deepseek-chat"]["skipped"]


def test_cli_canary_sin_keys_sale_1(monkeypatch, capsys):
    monkeypatch.setattr(PR, "call", lambda *a, **k: (_ for _ in ()).throw(
        PR.MissingKeyError("env var X_API_KEY not set")))
    from mmorch.cli import main
    assert main(["canary"]) == 1
    assert "faltan API keys" in capsys.readouterr().err


def test_baseline_y_drift(tmp_path):
    p = tmp_path / "baseline.json"
    alto = {"deepseek-chat": {"passed": 20, "total": 20, "pass_rate": 1.0,
                              "model_version": "v1", "failures": []}}
    rep = C.compare_baseline(alto, update=True, path=p)
    assert rep["baseline_updated"] and p.exists() and not rep["drift"]

    bajo = {"deepseek-chat": {"passed": 16, "total": 20, "pass_rate": 0.8,
                              "model_version": "v2", "failures": []}}
    rep2 = C.compare_baseline(bajo, path=p)
    r = rep2["models"]["deepseek-chat"]
    assert r["baseline"] == 1.0 and r["drift"] is True
    assert rep2["drift"] == ["deepseek-chat"]

    # caida chica (<= DRIFT_DROP) = ruido de sampling, no drift
    casi = {"deepseek-chat": {"passed": 18, "total": 20, "pass_rate": 0.9,
                              "model_version": "v2", "failures": []}}
    assert not C.compare_baseline(casi, path=p)["drift"]

    # un modelo salteado no toca baseline ni drift
    skip = {"gemini-2.5-flash": {"skipped": "falta key"}}
    assert not C.compare_baseline(skip, path=p)["drift"]
