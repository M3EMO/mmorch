"""sync: GitHub como bus entre maquinas. Un escritor (push a branch agente), auto-pull
seguro (solo arbol limpio, ff-only). git mockeado (no toca repos reales)."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.sync as SY


class _Git:
    """Fake git: scripteable por subcomando. Registra las llamadas."""
    def __init__(self, status="", push_rc=0, pull_rc=0):
        self.status = status; self.push_rc = push_rc; self.pull_rc = pull_rc; self.calls = []
    def __call__(self, repo, *args, timeout=120.0):
        self.calls.append(args)
        if args[:1] == ("status",):
            return 0, self.status
        if args[:1] == ("rev-parse",):
            return 0, "main"
        if args[:1] == ("push",):
            return self.push_rc, "" if self.push_rc == 0 else "rejected"
        if args[:1] == ("pull",):
            return self.pull_rc, "" if self.pull_rc == 0 else "not fast-forward"
        return 0, ""


def test_is_clean(monkeypatch):
    monkeypatch.setattr(SY, "_git", _Git(status=""))
    assert SY.is_clean("/r") is True
    monkeypatch.setattr(SY, "_git", _Git(status=" M app.py"))
    assert SY.is_clean("/r") is False


def test_commit_push_noop_when_clean(monkeypatch):
    g = _Git(status="")
    monkeypatch.setattr(SY, "_git", g)
    r = SY.commit_push("/r", "msg")
    assert r["pushed"] is False
    assert not any(a[:1] == ("push",) for a in g.calls)   # no pushea si limpio


def test_commit_push_pushes_to_agent_branch(monkeypatch):
    g = _Git(status=" M app.py", push_rc=0)
    monkeypatch.setattr(SY, "_git", g)
    r = SY.commit_push("/r", "fix bug")
    assert r["pushed"] is True and r["branch"] == SY.AUTO_BRANCH
    assert ("checkout", "-B", SY.AUTO_BRANCH) in g.calls   # branch del agente, NO main
    assert any(a[:2] == ("push", "-u") for a in g.calls)


def test_commit_push_reports_failure(monkeypatch):
    monkeypatch.setattr(SY, "_git", _Git(status=" M x", push_rc=1))
    assert SY.commit_push("/r", "m")["ok"] is False


def test_pull_skips_dirty_tree(monkeypatch):
    g = _Git(status=" M wip.py")
    monkeypatch.setattr(SY, "_git", g)
    r = SY.pull("/r")
    assert r["pulled"] is False and r["reason"] == "dirty"
    assert not any(a[:1] == ("pull",) for a in g.calls)   # nunca pisa WIP


def test_pull_ff_only_when_clean(monkeypatch):
    g = _Git(status="", pull_rc=0)
    monkeypatch.setattr(SY, "_git", g)
    r = SY.pull("/r")
    assert r["pulled"] is True
    assert any(a[:2] == ("pull", "--ff-only") for a in g.calls)


def test_pull_reports_divergence(monkeypatch):
    monkeypatch.setattr(SY, "_git", _Git(status="", pull_rc=1))
    r = SY.pull("/r")
    assert r["pulled"] is False   # no ff -> manual, no merge a ciegas


def test_pull_all_iterates_projects(monkeypatch):
    monkeypatch.setattr(SY, "_git", _Git(status="", pull_rc=0))
    monkeypatch.setattr("mmorch.projects.list_projects", lambda **k: {"a": "/ra", "b": "/rb"})
    out = SY.pull_all()
    assert set(out["pulled"]) == {"a", "b"} and all(v["pulled"] for v in out["pulled"].values())
