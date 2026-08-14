"""Tests F5 loop_nightly (contrato .scratch/loop-cerrado/spec.md)."""

import json

import mmorch.loop_nightly as ln
from mmorch.iohelpers import atomic_write_json


class FakeGen:
    def propose(self, payload):
        if "lente" in payload:
            return {"gist": None, "justification": "nada nuevo"}
        return {"score": 0.9, "justification": "aplica", "cited_file": None}


class FakeVer:
    def refute(self, payload):
        return {"refuted": False, "reason": ""}


def make_repo(tmp_path):
    (tmp_path / "logs").mkdir()
    (tmp_path / "vault" / "research").mkdir(parents=True)
    (tmp_path / "vault" / "roadmaps").mkdir()
    return tmp_path


def run(repo, **kw):
    kw.setdefault("generator", FakeGen())
    kw.setdefault("verifier", FakeVer())
    kw.setdefault("record_fn", lambda *a, **k: None)
    kw.setdefault("now_ts", 1000.0)
    return ln.run_idea_loop(repo_dir=str(repo), today="2026-08-14", **kw)


def test_loop_paused_skips_everything(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "logs" / "loop_paused").touch()
    assert run(repo) == {"skipped": "paused"}
    assert not (repo / "logs" / "loop_budget.json").exists()


def test_budget_exhausted_skips(tmp_path):
    repo = make_repo(tmp_path)
    atomic_write_json(repo / "logs" / "loop_budget.json",
                      {"month": "2026-08", "calls": ln.CAP_CALLS_PER_MONTH})
    assert run(repo) == {"skipped": "budget"}


def test_budget_old_month_resets(tmp_path):
    repo = make_repo(tmp_path)
    atomic_write_json(repo / "logs" / "loop_budget.json",
                      {"month": "2026-07", "calls": ln.CAP_CALLS_PER_MONTH})
    result = run(repo)
    assert "skipped" not in result
    budget = json.loads((repo / "logs" / "loop_budget.json").read_text())
    assert budget == {"month": "2026-08", "calls": 40}


def test_steps_run_and_state_updated(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    result = run(repo)
    assert result["errors"] == []
    for name in ("expire_ignored", "expire_candidates", "detect_promotions",
                 "adjudicate", "candidatas", "compose_cards"):
        assert name in result["steps"], name
    state = json.loads((repo / "logs" / "loop_state.json").read_text())
    assert state == {"last_run_ts": 1000.0}


def test_step_crash_is_isolated(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.outcomes.expire_ignored",
                        lambda **k: (_ for _ in ()).throw(RuntimeError("boom")))
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    result = run(repo)
    assert result["steps"]["expire_ignored"] is None
    assert any("expire_ignored" in e and "boom" in e for e in result["errors"])
    assert result["steps"]["compose_cards"] is not None  # los demas corrieron


def test_metrics_count_by_status(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    def m(n, status):
        return {"note_path": f"/v/{n}.md", "project": "p", "score": 0.9,
                "strong": True, "status": status, "shown_count": 0,
                "justification": "j", "cited_file": None, "id": f"/v/{n}.md|p"}
    pairs = {f"/v/{n}.md|p": {"hash": "h", "result": m(n, st)}
             for n, st in (("a", "pendiente"), ("b", "aceptada"),
                           ("c", "pendiente"))}
    atomic_write_json(repo / "logs" / "adjudications.json",
                      {"pairs": pairs, "by_project": {}})
    result = run(repo)
    assert result["metrics"]["por_status"] == {"pendiente": 2, "aceptada": 1}


def test_adjudication_runs_over_real_note(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    proj = tmp_path / "proyecto-x"
    proj.mkdir()
    monkeypatch.setattr("mmorch.projects._load",
                        lambda *a, **k: {"proyecto-x": str(proj)})
    (repo / "vault" / "research" / "nota.md").write_text(
        "---\ntitle: t\n---\nidea util", encoding="utf-8")
    result = run(repo)
    assert result["steps"]["adjudicate"]["judged"] == 1
    adj = json.loads((repo / "logs" / "adjudications.json").read_text(
        encoding="utf-8"))
    assert adj["by_project"]["proyecto-x"][0]["strong"] is True
    # compose_cards corrio despues: el strong match tiene card
    assert "card" in adj["by_project"]["proyecto-x"][0]


def test_no_fuel_no_candidates(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    atomic_write_json(repo / "logs" / "loop_state.json",
                      {"last_run_ts": 9999999999.0})
    result = run(repo)
    assert result["steps"]["candidatas"] == {"nuevas": 0, "sin_fuel": True}
