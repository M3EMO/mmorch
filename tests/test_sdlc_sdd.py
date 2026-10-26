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


def test_mergear_deja_la_mesa_fuera_de_main(tmp_path):
    """orchestration-eeq: main recibe codigo y docs/specs/; la review branch conserva docs/sdlc/ para reanudar."""
    import subprocess
    from sdlc.pipeline import mergear

    def g(*a):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *a], cwd=tmp_path, check=True,
                              capture_output=True, text=True).stdout

    def escribir(archivos):
        for rel, txt in archivos.items():
            (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
            (tmp_path / rel).write_text(txt, encoding="utf-8")
        g("add", "-A")

    g("init", "-q", "-b", "main")
    escribir({"a.py": "x = 1\n", "docs/sdlc/run-log.json": "{\"viejo\": 1}\n", "docs/sdlc/features/f.md": "ficha\n"})
    g("commit", "-q", "-m", "base con mesa vieja")
    g("checkout", "-q", "-b", "sdlc/f")
    escribir({"b.py": "y = 2\n", "docs/sdlc/run-log.json": "{\"stage\": 6}\n", "docs/sdlc/spec.md": "spec\n",
              "docs/sdlc/suite.log": "ok\n", "docs/specs/f/spec.md": "spec\n"})
    g("commit", "-q", "-m", "feature")
    g("checkout", "-q", "main")
    escribir({"docs/sdlc/run-log.json": "{\"otro\": 1}\n"})  # choque solo en la mesa: se resuelve sacandola
    g("commit", "-q", "-m", "otra corrida")

    ok, salida = mergear(str(tmp_path), "sdlc/f", "integra f")
    assert ok, salida
    en_main = set(g("ls-files").split())
    assert {"a.py", "b.py", "docs/specs/f/spec.md", "docs/sdlc/features/f.md"} <= en_main
    assert not {f"docs/sdlc/{n}" for n in ("run-log.json", "spec.md", "suite.log")} & en_main
    assert not (tmp_path / "docs/sdlc/run-log.json").exists()
    assert "docs/sdlc/run-log.json" in g("ls-tree", "-r", "--name-only", "sdlc/f").split()
    assert g("log", "-1", "--format=%s").strip() == "integra f"

    g("checkout", "-q", "-b", "sdlc/choque", "HEAD~1")
    escribir({"a.py": "x = 3\n"})
    g("commit", "-q", "-m", "choque en codigo")
    g("checkout", "-q", "main")
    escribir({"a.py": "x = 4\n"})
    g("commit", "-q", "-m", "main cambia a.py")
    assert not mergear(str(tmp_path), "sdlc/choque")[0]
    assert g("status", "--porcelain") == ""  # el choque fuera de la mesa aborta y deja main limpio
