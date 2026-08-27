"""W3.1 — health honesto: cada componente de EXPECTATIONS tiene emisor real
de beat() y el smoke se pone rojo cuando healthy=False (antes pasaba verde
con el sistema unhealthy, alarma entrenada a ignorarse)."""

import importlib.util
import json
import pathlib
import time

from mmorch import health

ROOT = pathlib.Path(__file__).resolve().parents[1]


def _beats(tmp_path):
    p = tmp_path / "health.jsonl"
    if not p.exists():
        return []
    return [json.loads(ln) for ln in p.read_text(encoding="utf-8").splitlines()]


def test_expectations_solo_componentes_con_emisor_real():
    # el contrato W3.1: declarar sin emisor esta prohibido; si esta lista
    # crece, el nuevo componente DEBE latir en algun camino vivo
    assert set(health.EXPECTATIONS) == {"nightly", "server", "digest"}


def test_start_health_beats_late_al_arrancar_y_periodico(tmp_path):
    from mmorch.server import start_health_beats
    stop = start_health_beats(logs_dir=str(tmp_path), interval_s=0.02)
    try:
        assert [b["component"] for b in _beats(tmp_path)][:1] == ["server"]
        assert _beats(tmp_path)[0]["detail"] == "startup"
        # espera acotada (no sleep fijo) a que el daemon repita el latido
        deadline = time.time() + 2.0
        while len(_beats(tmp_path)) < 3 and time.time() < deadline:
            time.sleep(0.01)
        beats = _beats(tmp_path)
        assert len(beats) >= 3
        assert {b["component"] for b in beats} == {"server"}
    finally:
        stop.set()


def test_write_local_digest_emite_beat_digest(tmp_path, monkeypatch):
    import mmorch.loop_nightly as ln
    monkeypatch.setattr(ln, "_llm_json", lambda *a, **k: {"digest": "todo ok"})
    r = ln.write_local_digest({"ts": 1.0}, logs_dir=str(tmp_path))
    assert (tmp_path / "digest_last.md").read_text(encoding="utf-8").strip() == "todo ok"
    assert r["chars"] == len("todo ok")
    assert [b["component"] for b in _beats(tmp_path)] == ["digest"]


def test_write_local_digest_sin_llm_no_late(tmp_path, monkeypatch):
    # el beat certifica digest ESCRITO: si el LLM revienta, cero latido
    import pytest
    import mmorch.loop_nightly as ln

    def boom(*a, **k):
        raise RuntimeError("llm caido")

    monkeypatch.setattr(ln, "_llm_json", boom)
    with pytest.raises(RuntimeError):
        ln.write_local_digest({"ts": 1.0}, logs_dir=str(tmp_path))
    assert _beats(tmp_path) == []


def _load_smoke():
    spec = importlib.util.spec_from_file_location(
        "smoke_script", ROOT / "scripts" / "smoke.py")
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def test_smoke_health_rojo_cuando_unhealthy(monkeypatch, capsys):
    smoke = _load_smoke()
    monkeypatch.setattr(
        "mmorch.health.report",
        lambda **k: {"healthy": False,
                     "check": {"dead": [{"component": "server"}],
                               "alive": [], "never": ["digest"]},
                     "errors": {"nightly_errors": {"x_error": "boom"},
                                "idea_loop_errors": []}})
    assert smoke.c_health() is False  # check() atrapa el raise -> fail del smoke
    assert "server" in smoke.WHY["health (report)"]


def test_smoke_health_verde_cuando_healthy(monkeypatch, capsys):
    smoke = _load_smoke()
    monkeypatch.setattr(
        "mmorch.health.report",
        lambda **k: {"healthy": True,
                     "check": {"dead": [], "alive": ["digest", "nightly", "server"],
                               "never": []},
                     "errors": {"nightly_errors": {}, "idea_loop_errors": []}})
    assert smoke.c_health() is True
