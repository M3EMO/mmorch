"""Seam Executor (W2.3): factory por env + coder loop probado con FakeExecutor inyectado
(sin invocar el claude CLI real — la seam ES donde el test mete el fake)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import pytest

import mmorch.claude_exec as CE
import mmorch.project_loop as PL


class FakeExecutor:
    """Graba las llamadas y devuelve un ExecResult fijo — cero subprocess."""

    def __init__(self, ok=True):
        self.calls = []
        self._ok = ok

    def run(self, prompt, cwd, *, mode="plan", timeout=600.0, job_id="", model=None):
        self.calls.append({"prompt": prompt, "cwd": cwd, "mode": mode, "job_id": job_id})
        return CE.ExecResult(ok=self._ok, result="fake", returncode=0, steps=3)


# ---- factory get_executor ----------------------------------------------------
def test_default_is_claude_cli(monkeypatch):
    monkeypatch.delenv("MMORCH_EXECUTOR", raising=False)
    assert isinstance(CE.get_executor(), CE.ClaudeCliExecutor)


def test_explicit_claude_cli(monkeypatch):
    monkeypatch.setenv("MMORCH_EXECUTOR", "claude-cli")
    assert isinstance(CE.get_executor(), CE.ClaudeCliExecutor)


@pytest.mark.parametrize("name", ["cursor-agent", "api"])
def test_future_backends_raise_not_implemented(monkeypatch, name):
    monkeypatch.setenv("MMORCH_EXECUTOR", name)
    with pytest.raises(NotImplementedError, match=name):
        CE.get_executor()


def test_unknown_backend_raises_value_error(monkeypatch):
    monkeypatch.setenv("MMORCH_EXECUTOR", "gpt-magico")
    with pytest.raises(ValueError, match="gpt-magico"):
        CE.get_executor()


# ---- ClaudeCliExecutor normaliza el dict de run_claude -----------------------
def test_claude_cli_executor_wraps_run_claude(monkeypatch):
    # dict PARCIAL a proposito: los fakes existentes en la suite devuelven {"ok": True}
    monkeypatch.setattr(CE, "run_claude", lambda *a, **k: {"ok": True})
    r = CE.ClaudeCliExecutor().run("hola", "/x", mode="edit")
    assert r == CE.ExecResult(ok=True, result="", returncode=None, steps=0)


def test_claude_cli_executor_bad_cwd_no_cli():
    # cwd inexistente corta ANTES de invocar el CLI -> seguro de correr en la suite
    r = CE.ClaudeCliExecutor().run("hola", "/no/such/dir")
    assert r.ok is False and r.returncode is None


# ---- coder loop via la seam (FakeExecutor inyectado, cero CLI) ---------------
def _setup_loop(monkeypatch, tmp_path):
    repo = tmp_path / "repo"; repo.mkdir()
    (repo / "app.py").write_text("def inc(x):\n    return x\n", encoding="utf-8")
    monkeypatch.setattr("mmorch.projects.resolve", lambda name, **k: str(repo))
    monkeypatch.setattr("mmorch.sync.commit_push", lambda *a, **k: {"pushed": True})
    monkeypatch.setattr("mmorch.providers.call",
                        lambda *a, **k: type("R", (), {"text": "```python\nx=1\n```"})())
    monkeypatch.setattr(PL, "_run_cmd", lambda cwd, cmd, timeout=120.0: (False, "rojo"))
    return repo


def test_coder_loop_escalates_through_fake_executor(monkeypatch, tmp_path):
    repo = _setup_loop(monkeypatch, tmp_path)
    fake = FakeExecutor(ok=True)
    monkeypatch.setattr("mmorch.claude_exec.get_executor", lambda: fake)
    r = PL.run_project_task("p", "tarea dura", target_file="app.py", test_cmd="pytest",
                            K=2, escalate=True)
    assert r.ok and r.escalated and r.engine == "claude"
    assert len(fake.calls) == 1
    c = fake.calls[0]
    assert c["prompt"] == "tarea dura" and c["cwd"] == str(repo) and c["mode"] == "edit"


def test_coder_loop_fake_executor_failure_propagates(monkeypatch, tmp_path):
    _setup_loop(monkeypatch, tmp_path)
    fake = FakeExecutor(ok=False)
    monkeypatch.setattr("mmorch.claude_exec.get_executor", lambda: fake)
    r = PL.run_project_task("p", "t", target_file="app.py", test_cmd="pytest",
                            K=1, escalate=True)
    assert not r.ok and r.escalated and len(fake.calls) == 1
