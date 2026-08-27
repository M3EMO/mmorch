"""W5.1 — contrato de error uniforme + huecos de borde (02-interface-contracts §4/§5).

Toda tool MCP registrada devuelve {"error": str, "kind": str} ante un fallo,
jamas una excepcion cruda ni formatos mixtos. El fallo controlado se inyecta via
wrapper.__wrapped__ (la seam del guard) — parametrizado sobre las 46 tools reales,
cero red y cero API. Los tests de hueco pegan en la LIBRERIA (donde vive la
validacion desde W5.1) y a traves del wrapper donde es barato.
"""
from __future__ import annotations

import inspect
import json

import pytest

from mmorch.mcp_server import mcp, _guarded, _kind_of

TOOLS = sorted(mcp._tool_manager.list_tools(), key=lambda t: t.name)
assert TOOLS, "el server no registro ninguna tool — el test no estaria probando nada"


@pytest.fixture(autouse=True)
def _telemetry_a_tmp(tmp_path, monkeypatch):
    # invocar tools dispara la telemetria MCP: redirigirla para no contaminar el
    # logs/mcp_calls.jsonl real (mismo defecto que 02 §5.5 documenta del selftest)
    import mmorch.mcp_telemetry as T
    monkeypatch.setattr(T, "_LOG", tmp_path / "mcp_calls.jsonl")


def _guard_of(fn):
    """El guard es el wrapper MAS PROFUNDO de la cadena __wrapped__ con el marker en
    su __dict__ propio (functools.wraps copia el marker hacia wrappers de afuera,
    p.ej. el de telemetria — por eso no alcanza mirar el objeto de arriba)."""
    guard, obj = None, fn
    while hasattr(obj, "__wrapped__"):
        if obj.__dict__.get("__mmorch_guarded__"):
            guard = obj
        obj = obj.__wrapped__
    assert guard is not None, f"{fn} no paso por _guarded"
    return guard


# ---------------------------------------------------------------------------
# contrato de error: cada tool registrada, fallo controlado -> {"error","kind"}
# ---------------------------------------------------------------------------
@pytest.mark.parametrize("tool", TOOLS, ids=lambda t: t.name)
def test_toda_tool_pasa_por_el_guard(tool):
    assert getattr(tool.fn, "__mmorch_guarded__", False), (
        f"{tool.name} se registro sin el guard de contrato de error")


@pytest.mark.parametrize("tool", TOOLS, ids=lambda t: t.name)
def test_fallo_controlado_devuelve_error_kind(tool, monkeypatch):
    def boom(*a, **k):
        raise RuntimeError("fallo controlado w5.1")
    monkeypatch.setattr(_guard_of(tool.fn), "__wrapped__", boom)
    out = json.loads(tool.fn())
    assert out == {"error": "fallo controlado w5.1", "kind": "internal"}, tool.name


def test_guard_preserva_firma_y_exito():
    # FastMCP arma el schema desde la firma: el guard no puede romperla
    def suma(a: int, b: int = 2) -> str:
        """doc original"""
        return json.dumps(a + b)
    g = _guarded(suma)
    assert inspect.signature(g) == inspect.signature(suma)
    assert g.__doc__ == "doc original"
    assert json.loads(g(3)) == 5  # exito pasa intacto


def test_kind_por_clase_de_excepcion():
    from mmorch.budget import BudgetExceeded
    casos = [
        (ValueError("v"), "invalid_input"),
        (KeyError("k"), "invalid_input"),
        (IndexError("i"), "invalid_input"),
        (TypeError("t"), "invalid_input"),
        (FileNotFoundError("f"), "not_found"),
        (PermissionError("p"), "io"),
        (RuntimeError("r"), "internal"),
        (BudgetExceeded("b"), "budget"),
    ]
    for exc, kind in casos:
        assert _kind_of(exc) == kind, exc


def test_error_vacio_cae_al_nombre_de_la_clase():
    def boom():
        raise RuntimeError()
    out = json.loads(_guarded(boom)())
    assert out == {"error": "RuntimeError", "kind": "internal"}


# ---------------------------------------------------------------------------
# huecos de borde (02 §4) — la validacion vive en la libreria, el wrapper adapta
# ---------------------------------------------------------------------------
def _tool_fn(name: str):
    return next(t.fn for t in TOOLS if t.name == name)


def test_hueco1_cascade_step_malformado():
    # antes: IndexError/ValueError crudos escapaban al framework MCP
    out = json.loads(_tool_fn("mmorch_cascade")("hola", steps=[["solo-modelo"]]))
    assert out["kind"] == "invalid_input" and "step invalido" in out["error"]
    out = json.loads(_tool_fn("mmorch_cascade")("hola", steps=[["m", "abc"]]))
    assert out["kind"] == "invalid_input"


def test_hueco2_check_checker_inexistente():
    out = json.loads(_tool_fn("mmorch_check")("no_existe", {}))
    assert out["kind"] == "invalid_input" and "arithmetic" in out["error"]
    # ctx con kwargs inesperados -> TypeError del checker, tambien contenida
    out = json.loads(_tool_fn("mmorch_check")("arithmetic", {"nope": 1}))
    assert out["kind"] == "invalid_input"


def test_hueco3_recall_bordes(tmp_path):
    from mmorch.memory import recall
    with pytest.raises(ValueError, match="k debe"):
        recall("q", k=0, path=tmp_path / "m.duckdb")
    with pytest.raises(ValueError, match="k debe"):
        recall("q", k=10_000, path=tmp_path / "m.duckdb")
    with pytest.raises(ValueError, match="window_days"):
        recall("q", window_days=-1, path=tmp_path / "m.duckdb")
    # via el wrapper: mismo fallo, contrato uniforme (raisea antes de tocar DB real)
    out = json.loads(_tool_fn("mmorch_recall")("q", k=0))
    assert out["kind"] == "invalid_input"


def test_hueco4_note_id_inexistente_ok_false(tmp_path):
    from mmorch import memory as M
    db = tmp_path / "m.duckdb"
    assert M.reinforce(99_999, path=db) is False
    assert M.flag_contradiction(99_999, path=db) is False
    assert M.close_loop(99_999, path=db) is False
    assert M.resolve_review(99_999, path=db) is False
    assert M.resolve_review(99_999, drop=True, path=db) is False
    nid = M.write_note("global", "nota real", path=db)
    assert M.reinforce(nid, path=db) is True
    assert M.flag_contradiction(nid, path=db) is True
    assert M.resolve_review(nid, path=db) is True


def test_hueco5_family_of_amable_y_threshold_clamp():
    from mmorch.config import family_of
    with pytest.raises(KeyError, match="known:"):
        family_of("modelo-inventado")
    # el clamp de route/cynefin es el mismo max/min: verificado por inspeccion de
    # la fuente (una llamada real gastaria API); aca se fija que exista el clamp
    # importlib: `import mmorch.route as R` devolveria la FUNCION route (el
    # __init__ del paquete pisa el atributo del modulo con el simbolo re-exportado)
    import importlib
    R = importlib.import_module("mmorch.route")
    C = importlib.import_module("mmorch.classify")
    assert "min(1.0, float(threshold))" in inspect.getsource(R.route)
    assert "min(1.0, float(threshold))" in inspect.getsource(C.cynefin_classify)


def test_hueco6_predicted_conf_clampada_al_escribir(tmp_path):
    from mmorch.feedback import record_outcome
    log = tmp_path / "fb.jsonl"
    record_outcome("arm", 1.0, predicted_conf=3.2, path=log)
    record_outcome("arm", 0.0, predicted_conf=-0.5, path=log)
    rows = [json.loads(x) for x in log.read_text(encoding="utf-8").splitlines()]
    assert rows[0]["predicted_conf"] == 1.0
    assert rows[1]["predicted_conf"] == 0.0


def test_hueco7_rubric_criteria_validada_en_el_borde():
    out = json.loads(_tool_fn("mmorch_rubric_start")("t", [{"desc": "sin id"}]))
    assert out["kind"] == "invalid_input" and "sin 'id'" in out["error"]
    out = json.loads(_tool_fn("mmorch_rubric_start")("t", ["no-dict"]))
    assert out["kind"] == "invalid_input"


def test_hueco8_review_gate_de_secretos(tmp_path):
    from mmorch.code_review import review_source
    # nombres nuevos cubiertos: id_rsa / .pem / .pfx
    for p in ("~/.ssh/id_rsa", "certs/server.pem", "win/store.pfx", ".env.prod"):
        with pytest.raises(ValueError, match="refused"):
            review_source(path=p)
    # contenido inline con credenciales tambien se frena (antes salia a la API)
    for code in ("-----BEGIN RSA PRIVATE KEY-----\nMII...",
                 "key = 'AKIAIOSFODNN7EXAMPLE'",
                 "token = 'ghp_" + "a" * 36 + "'"):
        with pytest.raises(ValueError, match="credential"):
            review_source(code=code)
    with pytest.raises(ValueError, match="no code"):
        review_source()
    # codigo limpio sigue pasando (find/refute inyectados: cero API)
    r = review_source("def f():\n    return 1\n", find=lambda: [],
                      refute=lambda fs: fs)
    assert r["n_confirmed"] == 0
    # via el wrapper: el refuse sale como {"error","kind"}, no como excepcion
    out = json.loads(_tool_fn("mmorch_review_code")(path="secrets/api.key"))
    assert out["kind"] == "invalid_input" and "refused" in out["error"]


def test_hueco9_evolve_self_sin_doble_strip():
    # el strip heuristico del wrapper (fragil con fences internos) se elimino:
    # propose_patch ya extrae el fence via textutil.extract_fence
    import mmorch.mcp_server as S
    src = inspect.getsource(S)
    assert 'a.split("```", 2)' not in src


def test_record_outcome_source_default_unificado():
    # W5.1 §5.4: el default MCP era "opus" y el de libreria "" — ahora coinciden
    fn = _tool_fn("mmorch_record_outcome").__wrapped__
    assert inspect.signature(fn).parameters["source"].default == ""
