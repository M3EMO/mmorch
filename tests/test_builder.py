"""mmorch.builder (builder-e2e ticket 06): bootstrap desde una carpeta vacia y encadenado de modulos. Cero API: un
build_feature falso crea la rama de cada modulo con un worktree real."""
import json
import subprocess

import pytest

import mmorch.builder as B
import mmorch.sdlc as S


def _git(repo, *a):
    return subprocess.run(["git", "-C", str(repo), *a], capture_output=True, text=True, check=True).stdout


def _plan(tmp_path, modulos):
    (tmp_path / "plan" / "c").mkdir(parents=True)
    (tmp_path / "plan" / "README.md").write_text("hola", encoding="utf-8")
    for m in modulos:
        (tmp_path / "plan" / "c" / f"{m}.test").write_text(f"test de {m}", encoding="utf-8")
    plan = {"bootstrap": {"README.md": "README.md"},
            "modulos": [{"nombre": m, "tarea": f"hace {m}", "files": [f"{m}.txt"], "contrato": f"c/{m}.test",
                         "accept": f"tests/{m}.test"} for m in modulos]}
    (tmp_path / "plan" / "p.json").write_text(json.dumps(plan), encoding="utf-8")
    return tmp_path / "plan" / "p.json"


def _falso(falla=()):
    vistos = []

    def build_feature(name, task, repo, *, accept, files, wt, phase):
        _git(repo, "worktree", "add", "-q", "-b", f"sdlc/{phase}", str(wt), "HEAD")
        vistos.append(sorted(p.name for p in wt.glob("*.txt")))   # lo que el modulo encontro al arrancar
        if name in falla:
            return {"status": "escalate", "failed_stage": "4-build", "failed_note": "rojo"}
        (wt / files[0]).write_text(task, encoding="utf-8")
        _git(wt, "add", "-A")
        _git(wt, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", name)
        return {"status": "built", "gate_rejects": []}
    return build_feature, vistos


def test_bootstrap_no_pisa_una_carpeta_con_contenido(tmp_path):
    (tmp_path / "repo").mkdir()
    (tmp_path / "repo" / "algo").write_text("x", encoding="utf-8")
    with pytest.raises(SystemExit, match="no esta vacia"):
        B.bootstrap(tmp_path / "repo", {})


def test_encadena_modulos_y_cada_uno_parte_del_anterior(tmp_path, monkeypatch):
    fake, vistos = _falso()
    monkeypatch.setattr(S, "build_feature", fake)
    filas = B.armar(str(_plan(tmp_path, ["m1", "m2", "m3"])), str(tmp_path / "repo"))
    assert [f["status"] for f in filas] == ["built"] * 3
    assert vistos == [[], ["m1.txt"], ["m1.txt", "m2.txt"]]   # el encadenado: m2 ve m1, m3 ve m1 y m2
    assert sorted(_git(tmp_path / "repo", "ls-files").split()) == ["README.md", "m1.txt", "m2.txt", "m3.txt"]


def test_corta_en_el_primer_modulo_que_no_se_construye(tmp_path, monkeypatch):
    fake, vistos = _falso(falla={"m2"})
    monkeypatch.setattr(S, "build_feature", fake)
    assert B.main([str(_plan(tmp_path, ["m1", "m2", "m3"])), str(tmp_path / "repo")]) == 1
    assert len(vistos) == 2   # m3 nunca corre
    assert "m2.txt" not in _git(tmp_path / "repo", "ls-files")
