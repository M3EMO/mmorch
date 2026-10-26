"""SDD en el SDLC: spec por feature (orchestration-3s7) y constitucion del repo (orchestration-fcr)."""
import types

import sdlc.pipeline as S


def _feature(tmp_path, name, texto):
    t = types.SimpleNamespace(name=name, task="x", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path)
    for n in ("spec", "plan"):
        S._write(f"docs/sdlc/{n}.md", f"{n} {texto}")
    S._archivar_spec()


def test_dos_features_dejan_dos_specs(tmp_path):
    _feature(tmp_path, "Tope de Retiro", "A")
    _feature(tmp_path, "Recarga parcial", "B")
    assert (tmp_path / "docs/specs/tope_de_retiro/spec.md").read_text(encoding="utf-8") == "spec A\n"
    assert (tmp_path / "docs/specs/tope_de_retiro/plan.md").read_text(encoding="utf-8") == "plan A\n"
    assert (tmp_path / "docs/specs/recarga_parcial/spec.md").read_text(encoding="utf-8") == "spec B\n"


def test_constitucion_entra_al_prompt_solo_si_existe(tmp_path, monkeypatch):
    # orchestration-fcr: sin el archivo, el prompt de spec y plan queda identico
    t = types.SimpleNamespace(name="f", task="x", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path)
    assert S._constitucion() == ""
    (tmp_path / "GOAL.md").write_text("Nunca un LLM juez para lo checkeable.\n", encoding="utf-8")
    assert "Nunca un LLM juez" in S._constitucion()
    monkeypatch.setitem(S.CFG, "constitution", "docs/reglas.md")
    assert S._constitucion() == ""


def test_write_no_sale_del_worktree(tmp_path):
    # orchestration-5s7: un plan con `..` o una ruta absoluta no escribe fuera del worktree
    import pytest
    t = types.SimpleNamespace(name="f", task="x", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path / "wt")
    for rel in ("pkg/../../fuera.py", str(tmp_path / "abs.py")):
        with pytest.raises(S.StageFailed):
            S._write(rel, "x = 1")
    assert not (tmp_path / "fuera.py").exists() and not (tmp_path / "abs.py").exists()
    S._write("pkg/a.py", "x = 1")
    assert (tmp_path / "wt/pkg/a.py").exists()
