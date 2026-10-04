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
    assert ok and S.state["plan_tamano"] == {"archivos": 8, "tokens": 1000, "escalon": "minima", "acople": 0,
                                             "lectores": []}


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


class _Corte(Exception):
    pass


def _acoplados(tmp_path):
    (tmp_path / "a.py").write_text('ROW = {"total_neto": 1, "moneda_iso": "ARS"}\n', encoding="utf-8")
    (tmp_path / "b.py").write_text('def lee(r):\n    return r["total_neto"], r["moneda_iso"]\n', encoding="utf-8")


def test_acople_cuenta_lectores_fuera_del_plan(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    _acoplados(tmp_path)
    t = S._tamano(["a.py"])
    assert t["acople"] == 1 and t["lectores"] == ["b.py"]
    assert S._tamano(["a.py", "b.py"])["acople"] == 0


def test_planner_y_coder_reciben_el_informe_de_impacto(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    _acoplados(tmp_path)
    sdlc = tmp_path / "docs" / "sdlc"
    sdlc.mkdir(parents=True)
    (sdlc / "spec.md").write_text("R1: a.py guarda total_neto en centavos.\n", encoding="utf-8")
    vistos = []

    def corta(*a, **k):
        vistos.append(a[-1])
        raise _Corte

    monkeypatch.setattr(S, "_planner", corta)
    try:
        S.plan()
    except _Corte:
        pass
    assert "INFORME DE IMPACTO" in vistos[-1] and "b.py" in vistos[-1]
    (sdlc / "plan.md").write_text("## Archivos\n- `a.py` [R1]\n", encoding="utf-8")
    S.state["plan_files"] = ["a.py"]
    monkeypatch.setattr(S, "llm", corta)
    try:
        S.build()
    except _Corte:
        pass
    assert "INFORME DE IMPACTO" in vistos[-1] and "b.py" in vistos[-1]


def test_plan_grande_deja_una_propuesta_de_division(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch)
    S.state["plan_tamano"] = {"archivos": 12}
    monkeypatch.setattr(S, "_planner", lambda system, user: "## Feature 1: base\n## Feature 2: lectores\n")
    S._proponer_division("spec", "plan")
    assert "## Feature 2" in (tmp_path / "docs" / "sdlc" / "division.md").read_text(encoding="utf-8")
