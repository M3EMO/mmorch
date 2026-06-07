"""Tests evolve (subset DGM gated): fitness, archive, propose. Subprocess/API mockeados."""
import sys, pathlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.evolve as EV
import mmorch.patterns as PAT
from mmorch.providers import CallResult


def _proc(out, rc=0):
    return types.SimpleNamespace(stdout=out, stderr="", returncode=rc)


def test_fitness_all_pass(monkeypatch):
    monkeypatch.setattr(EV.subprocess, "run", lambda *a, **k: _proc("30 passed in 1s", 0))
    f = EV.fitness()
    assert f["passed"] == 30 and f["failed"] == 0 and f["pass_rate"] == 1.0 and f["ok"] is True


def test_fitness_with_failures(monkeypatch):
    monkeypatch.setattr(EV.subprocess, "run", lambda *a, **k: _proc("28 passed, 2 failed in 1s", 1))
    f = EV.fitness()
    assert f["passed"] == 28 and f["failed"] == 2 and f["ok"] is False
    assert 0.9 < f["pass_rate"] < 0.94


def test_archive_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setattr(EV, "_ARCHIVE", tmp_path / "arch.jsonl")
    EV.archive_variant("v1", {"pass_rate": 1.0, "ok": True}, notes="fix X")
    EV.archive_variant("v2", {"pass_rate": 0.5, "ok": False})
    arch = EV.read_archive()
    assert len(arch) == 2 and arch[0]["name"] == "v1" and arch[1]["fitness"]["ok"] is False


def test_propose_patch_is_readonly(monkeypatch, tmp_path):
    # propose NO debe escribir; solo devuelve texto.
    monkeypatch.setattr(EV, "ROOT", tmp_path)
    (tmp_path / "mod.py").write_text("# old", encoding="utf-8")
    monkeypatch.setattr(PAT, "fan_out",
        lambda prompts, **k: [CallResult("deepseek-chat", "deepseek", "# NUEVO", 1, 1, 0.0, 0.0)])
    out = EV.propose_patch("mod.py", "mejorar X")
    assert out == "# NUEVO"
    assert (tmp_path / "mod.py").read_text(encoding="utf-8") == "# old"  # NO se modifico
