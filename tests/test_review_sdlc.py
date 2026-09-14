"""Gate de revision D11: hermeticidad de _codegraph_context (docs/sdlc/spec.md)."""
import shutil

from mmorch import project_loop as PL


def test_R1_sin_opt_in_no_invoca_shutil_which(monkeypatch, tmp_path):
    """Spec R1: sin .codegraph y sin opt-in, la funcion NO debe invocar shutil.which
    (ni _cg). El codigo actual resuelve `bin_` con shutil.which ANTES de chequear
    el opt-in, violando el contrato de docs/sdlc/spec.md."""
    monkeypatch.delenv("MMORCH_CODEGRAPH_AUTOINDEX", raising=False)
    monkeypatch.delenv("CODEGRAPH_BIN", raising=False)

    calls = []
    monkeypatch.setattr(shutil, "which", lambda *a, **k: calls.append(a) or None)

    repo = str(tmp_path / "repo")
    (tmp_path / "repo").mkdir()

    result = PL._codegraph_context(repo, "alguna tarea")

    assert result == ""
    assert calls == [], f"shutil.which fue invocado sin opt-in: {calls}"
