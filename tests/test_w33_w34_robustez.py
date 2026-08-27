"""W3.3+W3.4 — retry/backoff transitorio, half-open breaker por modelo, pool de
bucket_rank resiliente, rotacion de metrics.jsonl + budget multi-segmento, costo
estimado en timeout, price_asof, breaker USD por-run, summary() defensivo. Sin API."""
import json
import pathlib
import sys
import types

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import mmorch.bucketrank as BR
import mmorch.budget as B
import mmorch.metrics as MET
import mmorch.prices as PR
import mmorch.project_build as PB
import mmorch.project_integrate as PI
import mmorch.providers as PV


# ---------- infra fake ---------------------------------------------------------------
class _FakeUsage:
    prompt_tokens = 10
    completion_tokens = 20


class _FakeResp:
    def __init__(self, text="hi"):
        msg = types.SimpleNamespace(content=text)
        self.choices = [types.SimpleNamespace(message=msg)]
        self.usage = _FakeUsage()


class _Err429(Exception):
    status_code = 429


class _Err503(Exception):
    status_code = 503


class APITimeoutError(Exception):
    """Nombre con 'timeout' a proposito: _classify_error clasifica por duck-typing."""


class _SeqClient:
    """Cliente fake: lanza las excepciones de `seq` en orden; agotadas, devuelve exito."""
    def __init__(self, seq=()):
        self._seq = list(seq)
        self.calls = 0
        self.chat = types.SimpleNamespace(
            completions=types.SimpleNamespace(create=self._create))

    def _create(self, **kw):
        self.calls += 1
        if self._seq:
            raise self._seq.pop(0)
        return _FakeResp()


@pytest.fixture(autouse=True)
def _aislado(monkeypatch, tmp_path):
    """Breaker limpio, sin sleeps reales, metrics/budget a tmp, sin techo mensual."""
    monkeypatch.setattr(PV, "_BREAKERS", {})
    monkeypatch.setattr(PV, "_sleep", lambda s: None)
    monkeypatch.setattr(MET, "_LOG_DIR", tmp_path)
    monkeypatch.setattr(MET, "_LOG_PATH", tmp_path / "metrics.jsonl")
    monkeypatch.setattr(B, "_SPEND_CACHE", {})
    # caches de plan/codigo a tmp: sin esto un run anterior deja unit-code cacheado en
    # logs/ real y build_unit sirve el hit SIN llamar al coder -> el usd-breaker (que
    # vive en el wrapper de gen) nunca corre y el test se vuelve orden-dependiente.
    monkeypatch.setattr(PB, "_WORKLIST_CACHE", tmp_path / "wl_cache.json")
    monkeypatch.setattr(PB, "_UNIT_CODE_CACHE", tmp_path / "unit_code_cache.json")
    monkeypatch.delenv("MMORCH_MAX_MONTHLY_USD", raising=False)
    monkeypatch.delenv("MMORCH_MAX_USD_PER_RUN", raising=False)


@pytest.fixture
def eventos(monkeypatch):
    out = []
    monkeypatch.setattr(PV, "log_event", lambda **rec: out.append(rec))
    return out


# ---------- W3.3: retry con backoff -------------------------------------------------
def test_retry_429_transitorio_termina_verde(monkeypatch, eventos):
    fc = _SeqClient([_Err429("429"), _Err429("429")])
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    dormido = []
    monkeypatch.setattr(PV, "_sleep", lambda s: dormido.append(s))
    r = PV.call("deepseek-chat", "hola")
    assert r.text == "hi" and fc.calls == 3
    errs = [e for e in eventos if e.get("error")]
    assert len(errs) == 2 and all(e["error_class"] == "rate_limit" for e in errs)
    assert all(e["retried"] for e in errs)          # cada retry quedo loggeado
    assert len(dormido) == 2 and dormido[1] > dormido[0]   # backoff exponencial

def test_5xx_es_transitorio(monkeypatch, eventos):
    fc = _SeqClient([_Err503("503 unavailable")])
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    assert PV.call("deepseek-chat", "hola").text == "hi"
    assert fc.calls == 2


def test_error_no_transitorio_no_reintenta(monkeypatch, eventos):
    fc = _SeqClient([ValueError("bad request 400")])
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    with pytest.raises(ValueError):
        PV.call("deepseek-chat", "hola")
    assert fc.calls == 1                            # cero retries
    assert eventos[-1]["retried"] is False


def test_retry_agotado_relanza(monkeypatch, eventos):
    fc = _SeqClient([_Err429("429")] * 5)
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    with pytest.raises(_Err429):
        PV.call("deepseek-chat", "hola")
    assert fc.calls == PV._RETRY_MAX_ATTEMPTS       # max 3 intentos, ni uno mas


# ---------- W3.3: half-open breaker -------------------------------------------------
def test_breaker_abre_tras_fallos_seguidos(monkeypatch, eventos):
    fc = _SeqClient([_Err429("429")] * 10)
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    with pytest.raises(_Err429):
        PV.call("deepseek-chat", "hola")            # 3 fallos -> abre
    calls_previas = fc.calls
    with pytest.raises(PV.BreakerOpen):
        PV.call("deepseek-chat", "hola")            # fail-fast, ni toca el cliente
    assert fc.calls == calls_previas
    assert eventos[-1]["error_class"] == "breaker_open"   # el fail-fast queda medido


def test_breaker_half_open_probe_cierra(monkeypatch):
    fc = _SeqClient([_Err429("429")] * 3)           # 3 fallos, despues exito
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    with pytest.raises(_Err429):
        PV.call("deepseek-chat", "hola")
    reloj = {"t": 0.0}
    monkeypatch.setattr(PV, "_now", lambda: reloj["t"])
    reloj["t"] = PV._BREAKER_COOLDOWN_S + 1e9       # cooldown holgadamente vencido
    r = PV.call("deepseek-chat", "hola")            # probe -> exito -> cierra
    assert r.text == "hi"
    assert PV._BREAKERS["deepseek-chat"]["opened_at"] is None
    PV.call("deepseek-chat", "hola")                # cerrado: fluye normal


def test_breaker_probe_fallido_reabre(monkeypatch):
    fc = _SeqClient([_Err429("429")] * 10)
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    reloj = {"t": 0.0}
    monkeypatch.setattr(PV, "_now", lambda: reloj["t"])
    with pytest.raises(_Err429):
        PV.call("deepseek-chat", "hola")            # 3 fallos en t=0 -> abre
    reloj["t"] = PV._BREAKER_COOLDOWN_S + 1
    with pytest.raises(_Err429):
        PV.call("deepseek-chat", "hola")            # probe falla -> re-abre
    assert PV._BREAKERS["deepseek-chat"]["opened_at"] == reloj["t"]
    with pytest.raises(PV.BreakerOpen):
        PV.call("deepseek-chat", "hola")            # cooldown fresco: fail-fast de nuevo


# ---------- W3.4: timeout loggea costo ESTIMADO -------------------------------------
def test_timeout_loggea_costo_estimado(monkeypatch, eventos):
    fc = _SeqClient([APITimeoutError("request timed out")] * 5)
    monkeypatch.setattr(PV, "_client", lambda mk: fc)
    with pytest.raises(APITimeoutError):
        PV.call("deepseek-chat", "x" * 4000)        # ~1000 tokens enviados
    errs = [e for e in eventos if e.get("error")]
    assert errs and all(e["error_class"] == "timeout" for e in errs)
    assert all(e["cost_usd"] > 0 for e in errs), "cost=0 en timeout subestima el gasto"
    assert all(e["cost_estimated"] and e["in_tokens"] > 0 for e in errs)


# ---------- W3.3: pool de bucket_rank no aborta por un item -------------------------
def test_bucketrank_pool_sobrevive_excepcion(monkeypatch):
    def _fake_call(model, msgs, **kw):
        if "boom" in msgs[-1]["content"]:
            raise RuntimeError("api reventada")
        return types.SimpleNamespace(text="razonado...\nTIER: A", cost_usd=0.001)

    monkeypatch.setattr(BR, "call", _fake_call)
    r = BR.bucket_rank(["uno", "boom aca", "tres"], rubric="calidad")
    assert r.n_failed == 1
    assert sorted(r.by_tier["A"]) == ["tres", "uno"]
    assert r.by_tier["D"] == ["boom aca"]           # dropeado al tier mas bajo, no perdido
    assert len(r.graded) == 3


# ---------- W3.4: rotacion de metrics.jsonl + budget multi-segmento -----------------
def test_rotacion_y_budget_rederiva_segmentos(monkeypatch, tmp_path):
    mes = __import__("datetime").datetime.now().strftime("%Y-%m")
    iso = f"{mes}-01T10:00:00"
    # segmento "viejo" grande ya en el path activo, con gasto del mes
    MET._LOG_PATH.write_text(
        json.dumps({"iso": iso, "cost_usd": 1.0, "model": "m", "family": "f"}) + "\n"
        + "x" * 300, encoding="utf-8")
    monkeypatch.setattr(MET, "_ROTATE_MAX_BYTES", 100)
    MET.log_event(pattern="t", node="n", model="m", family="f", in_tokens=1,
                  out_tokens=1, cost_usd=2.0, latency_s=0.1)
    rotados = MET.rotated_paths()
    assert len(rotados) == 1, "archivo >50MB (fake 100B) debe rotarse con fecha"
    assert MET._LOG_PATH.read_text(encoding="utf-8").count("\n") == 1
    # budget re-deriva el mes desde TODOS los segmentos (1.0 rotado + 2.0 activo)
    assert B.monthly_spend(mes) == pytest.approx(3.0)


def test_summary_defensivo_con_linea_incompleta(monkeypatch):
    MET.log_event(pattern="t", node="n", model="m", family="f", in_tokens=1,
                  out_tokens=1, cost_usd=0.5, latency_s=0.1)
    with open(MET._LOG_PATH, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"model": "solo-model"}) + "\n")   # sin cost/family
    s = MET.summary()
    assert s["calls"] == 2 and s["total_cost_usd"] == pytest.approx(0.5)
    assert s["calls_by_model"]["solo-model"] == 1


# ---------- W3.4: price_asof --------------------------------------------------------
def test_price_asof_vencido_warnea_una_vez(monkeypatch, tmp_path, caplog):
    p = tmp_path / "prices.json"
    p.write_text(json.dumps({"deepseek-chat": {"price_in": 0.1, "price_out": 0.2,
                                               "price_asof": "2020-01-01"}}),
                 encoding="utf-8")
    monkeypatch.setattr(PR, "_WARNED", set())
    with caplog.at_level("WARNING", logger="mmorch.prices"):
        assert PR.effective_prices("deepseek-chat", path=p) == (0.1, 0.2)
        PR.effective_prices("deepseek-chat", path=p)
    avisos = [r for r in caplog.records if "price_asof" in r.getMessage()]
    assert len(avisos) == 1, "warning una vez por modelo por proceso, sin spam"


def test_price_asof_fresco_no_warnea(monkeypatch, tmp_path, caplog):
    hoy = __import__("datetime").datetime.now().strftime("%Y-%m-%d")
    p = tmp_path / "prices.json"
    p.write_text(json.dumps({"deepseek-chat": {"price_in": 0.1, "price_out": 0.2,
                                               "price_asof": hoy}}), encoding="utf-8")
    monkeypatch.setattr(PR, "_WARNED", set())
    with caplog.at_level("WARNING", logger="mmorch.prices"):
        PR.effective_prices("deepseek-chat", path=p)
    assert not [r for r in caplog.records if "price_asof" in r.getMessage()]


# ---------- W3.4: breaker USD por-run en build_project ------------------------------
def _fakes():
    return dict(gen=lambda u, fb: "def u():\n    return 1",
                run_test=lambda u, c, tc: (True, "green"),
                run_snippet=lambda c, a: (True, ""), propose_test=lambda c, s: "",
                integrate=lambda e, rs: (True, "ok"), commit=lambda n, rr: None)


def test_usd_breaker_corta_el_run():
    r = PI.build_project(
        "t", "/nonexistent", external_test=None, max_usd_per_run=1.0,
        run_cost=lambda: 99.0,   # costo fake ya excedido
        plan=lambda t, e: [{"name": "u", "spec": "s", "deps": [], "test_cmd": "pytest -q"}],
        **_fakes())
    assert r["status"] == "escalate", r


def test_usd_breaker_default_por_env(monkeypatch):
    monkeypatch.setenv("MMORCH_MAX_USD_PER_RUN", "0.5")
    r = PI.build_project(
        "t", "/nonexistent", external_test=None, run_cost=lambda: 0.6,
        plan=lambda t, e: [{"name": "u", "spec": "s", "deps": [], "test_cmd": "pytest -q"}],
        **_fakes())
    assert r["status"] == "escalate", r


def test_usd_bajo_el_techo_no_corta():
    r = PI.build_project(
        "t", "/nonexistent", external_test=None, max_usd_per_run=1.0,
        run_cost=lambda: 0.01,
        plan=lambda t, e: [{"name": "u", "spec": "s", "deps": [], "test_cmd": "pytest -q"}],
        **_fakes())
    assert r["status"] == "built", r
    assert "run_usd" in r["provenance"]
