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
