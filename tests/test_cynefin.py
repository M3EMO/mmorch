"""Cynefin triage (P1): mapea request a dominio + estrategia, gate de escalada.
Invariante clave: 'chaotic' escala a Opus AUNQUE la confianza sea alta (en caos la
jugada es actuar ya, no rutear barato)."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
C = importlib.import_module("mmorch.classify")  # modulo, no la fn (shadow en __init__)


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.001


def _stub(monkeypatch, domain, conf):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res(f"CLASS: {domain}\nCONFIDENCE: {conf}"))


def test_clear_recommends_direct_cheap(monkeypatch):
    _stub(monkeypatch, "clear", 0.95)
    r = C.cynefin_classify("seguir la receta paso a paso")
    assert r.domain == "clear" and r.strategy == "direct_cheap" and not r.escalate


def test_complex_recommends_fanout(monkeypatch):
    _stub(monkeypatch, "complex", 0.9)
    r = C.cynefin_classify("integrar dos culturas de empresa")
    assert r.domain == "complex" and r.strategy == "fan_out+ensemble_verify" and not r.escalate


def test_chaotic_escalates_even_high_conf(monkeypatch):
    # gate clave: alta confianza NO evita la escalada en caos.
    _stub(monkeypatch, "chaotic", 0.99)
    r = C.cynefin_classify("crisis: producto contaminado, no se cuantos afectados")
    assert r.domain == "chaotic" and r.escalate and r.strategy == "escalate_opus"


def test_low_confidence_escalates(monkeypatch):
    _stub(monkeypatch, "complicated", 0.3)
    r = C.cynefin_classify("ambiguo", threshold=0.6)
    assert r.escalate and r.strategy == "escalate_opus"


def test_invalid_domain_escalates(monkeypatch):
    _stub(monkeypatch, "inventado", 0.9)
    r = C.cynefin_classify("x")
    assert r.domain is None and r.escalate
