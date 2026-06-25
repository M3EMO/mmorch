"""2il: storage-vs-manipulation triage. Physics-of-LLMs: loopear agrega computo, no
conocimiento. storage -> route_up (loops no ayudan); manipulation -> loop_budget.
Opt-in, library-first (no auto-default). Mismo gate de escalada que cynefin."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
C = importlib.import_module("mmorch.classify")


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.001


def _stub(monkeypatch, cls, conf):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res(f"CLASS: {cls}\nCONFIDENCE: {conf}"))


def test_storage_recommends_route_up(monkeypatch):
    _stub(monkeypatch, "storage", 0.9)
    r = C.task_type_classify("en que anio nacio Turing")
    assert r.task_type == "storage" and r.strategy == "route_up" and not r.escalate


def test_manipulation_recommends_loop_budget(monkeypatch):
    _stub(monkeypatch, "manipulation", 0.9)
    r = C.task_type_classify("dado A>B y B>C, ordena y explica")
    assert r.task_type == "manipulation" and r.strategy == "loop_budget" and not r.escalate


def test_low_confidence_escalates(monkeypatch):
    _stub(monkeypatch, "manipulation", 0.3)
    r = C.task_type_classify("ambiguo", threshold=0.6)
    assert r.escalate and r.strategy == "escalate_opus"


def test_invalid_type_escalates(monkeypatch):
    _stub(monkeypatch, "inventado", 0.9)
    r = C.task_type_classify("x")
    assert r.task_type is None and r.escalate
