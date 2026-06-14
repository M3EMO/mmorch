"""project_loop: ejecutor mmorch-primario (DeepSeek genera + tests verifican + aplica), con
escalada a claude. providers.call + tests + git mockeados; repo = tmp dir real."""
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.project_loop as PL


def _setup(monkeypatch, tmp_path, gen_seq, test_results):
    repo = tmp_path / "repo"; repo.mkdir()
    (repo / "app.py").write_text("def inc(x):\n    return x\n", encoding="utf-8")
    monkeypatch.setattr("mmorch.projects.resolve", lambda name, **k: str(repo))
    monkeypatch.setattr("mmorch.sync._git", lambda *a, **k: (0, ""))   # importado dentro de la fn
    monkeypatch.setattr("mmorch.sync.commit_push", lambda *a, **k: {"pushed": True})
    gen = iter(gen_seq)
    monkeypatch.setattr("mmorch.providers.call",
                        lambda *a, **k: type("R", (), {"text": next(gen)})())
    tr = iter(test_results)
    monkeypatch.setattr(PL, "_run_cmd", lambda cwd, cmd, timeout=120.0: (next(tr), "out"))
    return repo


def test_mmorch_solves_after_correction(monkeypatch, tmp_path):
    repo = _setup(monkeypatch, tmp_path,
                  gen_seq=["```python\ndef inc(x):\n    return x\n```",        # falla
                           "```python\ndef inc(x):\n    return x+1\n```"],      # pasa
                  test_results=[False, True])
    r = PL.run_project_task("p", "inc debe sumar 1", target_file="app.py",
                            test_cmd="pytest", K=4)
    assert r.ok and r.engine == "mmorch" and r.iterations == 2 and r.pushed
    assert "return x+1" in (repo / "app.py").read_text(encoding="utf-8")  # aplicado al repo


def test_escalates_to_claude_after_K(monkeypatch, tmp_path):
    _setup(monkeypatch, tmp_path,
           gen_seq=["```python\nx=1\n```"] * 3, test_results=[False, False, False])
    called = {}
    monkeypatch.setattr("mmorch.claude_exec.run_claude",
                        lambda task, cwd, **k: called.update(task=task) or {"ok": True})
    r = PL.run_project_task("p", "tarea dura", target_file="app.py", test_cmd="pytest",
                            K=3, escalate=True)
    assert r.escalated and r.engine == "claude" and r.ok
    assert called.get("task") == "tarea dura"


def test_no_escalation_returns_failure(monkeypatch, tmp_path):
    _setup(monkeypatch, tmp_path, gen_seq=["```python\nx=1\n```"] * 2, test_results=[False, False])
    r = PL.run_project_task("p", "t", target_file="app.py", test_cmd="pytest", K=2, escalate=False)
    assert not r.ok and r.engine == "mmorch" and not r.escalated


def test_no_test_cmd_breaks_unverified(monkeypatch, tmp_path):
    repo = _setup(monkeypatch, tmp_path, gen_seq=["```python\ndef inc(x):\n    return x+1\n```"],
                  test_results=[])
    r = PL.run_project_task("p", "t", target_file="app.py", test_cmd=None, K=3)
    # sin test_cmd no verifica -> escribe y corta (no confirma ok)
    assert "return x+1" in (repo / "app.py").read_text(encoding="utf-8")
