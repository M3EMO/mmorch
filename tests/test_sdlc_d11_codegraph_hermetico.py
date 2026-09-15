"""D11 (robustez: `project_loop._codegraph_context` no dispara indexadores externos por sorpresa).

Medido 2026-09-14: con `codegraph` en el PATH, un test sobre un repo temporal disparo `codegraph init`
+ `index` reales; el proceso del indexador dejo `.codegraph/codegraph.db` abierto y pytest no pudo borrar
su basetemp -> 594 ERROR de setup en la suite. Reglas:
- R1 sin `.codegraph` previo, NO se corre `init`/`index` salvo opt-in explicito MMORCH_CODEGRAPH_AUTOINDEX=1
  (default nuevo: 0). Devuelve '' sin tocar el repo.
- R2 con opt-in, un repo bajo el directorio temporal del sistema (tempfile.gettempdir()) TAMPOCO se indexa.
- R3 con `.codegraph` ya presente, `sync` sigue corriendo como hoy (comportamiento conservado).
Cero binario real: `_cg` y `shutil.which` se parchean.
"""
import shutil
import tempfile
from pathlib import Path

import mmorch.project_loop as PL


def _fake_cg(calls):
    class _P:
        returncode = 0
        stdout = ""
        stderr = ""

    def cg(bin_, repo, sub, *, arg="", timeout=60.0):
        calls.append(sub)
        return _P()
    return cg


def _repo(tmp_path: Path) -> Path:
    d = tmp_path / "repo"
    d.mkdir()
    (d / "a.py").write_text("x = 1\n", encoding="utf-8")
    return d


def test_R1_sin_indice_previo_no_indexa_sin_opt_in(tmp_path, monkeypatch):
    calls: list = []
    monkeypatch.setattr(PL, "_cg", _fake_cg(calls))
    monkeypatch.setattr(shutil, "which", lambda name: r"C:\bin\codegraph.cmd")
    monkeypatch.delenv("MMORCH_CODEGRAPH_AUTOINDEX", raising=False)
    monkeypatch.delenv("CODEGRAPH_BIN", raising=False)
    repo = _repo(tmp_path)
    out = PL._codegraph_context(str(repo), "tarea")
    assert out == "" and calls == [], (out, calls)
    assert not (repo / ".codegraph").exists()


def test_R2_con_opt_in_pero_repo_temporal_no_indexa(monkeypatch):
    calls: list = []
    monkeypatch.setattr(PL, "_cg", _fake_cg(calls))
    monkeypatch.setenv("CODEGRAPH_BIN", r"C:\bin\codegraph.cmd")
    monkeypatch.setenv("MMORCH_CODEGRAPH_AUTOINDEX", "1")
    d = Path(tempfile.mkdtemp()) / "repo"
    d.mkdir()
    out = PL._codegraph_context(str(d), "tarea")
    assert out == "" and calls == [], (out, calls)


def test_R3_con_indice_presente_sincroniza(tmp_path, monkeypatch):
    calls: list = []
    monkeypatch.setattr(PL, "_cg", _fake_cg(calls))
    monkeypatch.setenv("CODEGRAPH_BIN", r"C:\bin\codegraph.cmd")
    repo = _repo(tmp_path)
    (repo / ".codegraph").mkdir()
    PL._codegraph_context(str(repo), "tarea")
    assert calls and calls[0] == "sync", calls
