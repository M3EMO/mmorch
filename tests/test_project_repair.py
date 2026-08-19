"""Tests de project_repair (fakes, sin engine ni git real salvo el minimo)."""

import json
import subprocess

from mmorch.project_repair import failing_projects, repair_projects


def test_failing_projects_only_failing_not_timeouts():
    rec = {"project_health": {"failing": ["Portfolio financiero"],
                              "errors": ["Otro: TimeoutExpired ..."],
                              "ok": ["x"]}}
    assert failing_projects(rec) == ["Portfolio financiero"]
    assert failing_projects({}) == []


def _setup(tmp_path, monkeypatch, failing_name, project_dir):
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "nightly.jsonl").write_text(
        json.dumps({"project_health": {"failing": [failing_name]}}) + "\n",
        encoding="utf-8")
    monkeypatch.setattr("mmorch.projects._load",
                        lambda *a, **k: {failing_name: str(project_dir)})
    return logs


def test_repair_skips_non_git_project(tmp_path, monkeypatch):
    proj = tmp_path / "p"
    proj.mkdir()  # sin .git
    _setup(tmp_path, monkeypatch, "p", proj)
    r = repair_projects(str(tmp_path), today="2026-08-19")
    assert "sin objetivo elegible" in r["skipped"]


def test_repair_builds_in_project_worktree(tmp_path, monkeypatch):
    proj = tmp_path / "p"
    proj.mkdir()
    subprocess.run(["git", "init", "-b", "main"], cwd=proj, capture_output=True)
    subprocess.run(["git", "config", "user.email", "t@t"], cwd=proj, capture_output=True)
    subprocess.run(["git", "config", "user.name", "t"], cwd=proj, capture_output=True)
    (proj / "a.py").write_text("a = 1\n", encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=proj, capture_output=True)
    subprocess.run(["git", "commit", "-m", "base", "--no-verify"], cwd=proj,
                   capture_output=True)
    _setup(tmp_path, monkeypatch, "p", proj)
    seen = {}

    def fake_build(task, wt_path, gate):
        seen["task"] = task
        seen["wt"] = wt_path
        return {"status": "built"}

    r = repair_projects(str(tmp_path), today="2026-08-19", build_fn=fake_build)
    assert r["status"] == "built" and r["project"] == "p"
    assert "p" in seen["task"] and "ROJA" in seen["task"]
    assert str(proj) not in seen["wt"]  # worktree aislado, no el repo vivo
    # branch de review quedo en el repo del proyecto
    out = subprocess.run(["git", "branch", "--list", "mmorch/sana*"],
                         cwd=proj, capture_output=True, text=True)
    assert "mmorch/sana" in out.stdout
    # retry window persistida
    state = json.loads((tmp_path / "logs" / "project_repair_state.json")
                       .read_text(encoding="utf-8"))
    assert state["p"]["result"] == "built"
    # segunda corrida: en ventana -> skip
    r2 = repair_projects(str(tmp_path), today="2026-08-20", build_fn=fake_build)
    assert "skipped" in r2


def test_repair_paused(tmp_path, monkeypatch):
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "loop_paused").touch()
    assert repair_projects(str(tmp_path), today="2026-08-19") == {"skipped": "paused"}


def test_repair_logea_el_motivo_cuando_falla(tmp_path, monkeypatch):
    """Antes se guardaba SOLO el status: el 'detail' real de build_project()
    (p.ej. interface mismatch en la integracion) se perdia sin loguearse."""
    proj = tmp_path / "p"
    proj.mkdir()
    subprocess.run(["git", "init", "-b", "main"], cwd=proj, capture_output=True)
    subprocess.run(["git", "config", "user.email", "t@t"], cwd=proj, capture_output=True)
    subprocess.run(["git", "config", "user.name", "t"], cwd=proj, capture_output=True)
    (proj / "a.py").write_text("a = 1\n", encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=proj, capture_output=True)
    subprocess.run(["git", "commit", "-m", "base", "--no-verify"], cwd=proj,
                   capture_output=True)
    logs = _setup(tmp_path, monkeypatch, "p", proj)

    def fake_build(task, wt_path, gate):
        return {"status": "integration_failed", "detail": "3 failed: interface mismatch"}

    r = repair_projects(str(tmp_path), today="2026-08-19", build_fn=fake_build)
    assert r["status"] == "integration_failed"
    assert r["detail"] == "3 failed: interface mismatch"

    linea = json.loads((logs / "project_repair.jsonl").read_text(
        encoding="utf-8").strip().splitlines()[-1])
    assert linea["detail"] == "3 failed: interface mismatch"
    assert linea["project"] == "p"
