"""orchestration-e4d: baranda de tamaño en la etapa 3. Frena un plan fuera del rango medido (mas de 8 archivos o mas
de 16k tokens de archivos existentes) y deja el tamaño en el run-log. Cero API."""
import types

import mmorch.sdlc as S


def _wt(tmp_path, monkeypatch, toml=""):
    (tmp_path / "sdlc.toml").write_text(toml, encoding="utf-8")
    t = types.SimpleNamespace(name="t", task="tamano", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [], "suite": []}, wt=tmp_path,
                accept_cmd="python -c \"pass\"")
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")


def test_dentro_del_rango_pasa_y_registra(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    (tmp_path / "a.py").write_text("x" * 4000, encoding="utf-8")   # 1000 tokens existentes
    files = ["a.py"] + [f"n{i}.py" for i in range(7)]               # 8 archivos: el maximo observado
    ok, note = S.gate_tamano(files)
    assert ok and S.state["plan_tamano"] == {"archivos": 8, "tokens": 1000, "escalon": "minima"}


def test_fuera_del_rango_frena_y_pide_dividir(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    ok, note = S.gate_tamano([f"n{i}.py" for i in range(9)])
    assert not ok and "dividi la tarea" in note
    (tmp_path / "grande.py").write_text("x" * 70000, encoding="utf-8")   # 17500 tokens
    ok, note = S.gate_tamano(["grande.py"])
    assert not ok and S.state["plan_tamano"]["escalon"] == "media"


def test_umbral_por_repo(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch, "tamano_max_archivos = 12\n")
    assert S.gate_tamano([f"n{i}.py" for i in range(10)])[0]
