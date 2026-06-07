"""classify_and_act: triage barato + dispatch a handler + escalate gating."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
C = importlib.import_module("mmorch.classify")  # modulo, no la fn (shadow en __init__)

CLASSES = {"bulk": "muchas tareas independientes", "choose": "elegir mejor opcion",
           "verify": "chequear correctitud"}


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.001


def test_classifies_and_dispatches(monkeypatch):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res("razon\nCLASS: bulk\nCONFIDENCE: 0.9"))
    hit = {}
    def _h(req):
        hit["v"] = req
        return "DONE"
    r = C.classify_and_act("procesa 100 items", classes=CLASSES, handlers={"bulk": _h})
    assert r.cls == "bulk" and r.handled and not r.escalate
    assert r.result == "DONE" and hit["v"] == "procesa 100 items"


def test_low_confidence_escalates(monkeypatch):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res("CLASS: bulk\nCONFIDENCE: 0.3"))
    r = C.classify_and_act("ambiguo", classes=CLASSES,
                           handlers={"bulk": lambda req: "X"}, threshold=0.6)
    assert r.escalate and not r.handled and r.result is None


def test_no_handler_escalates(monkeypatch):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res("CLASS: verify\nCONFIDENCE: 0.95"))
    r = C.classify_and_act("revisa esto", classes=CLASSES, handlers={"bulk": lambda r: "X"})
    assert r.cls == "verify" and r.escalate and not r.handled


def test_invalid_class_escalates(monkeypatch):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res("CLASS: inventada\nCONFIDENCE: 0.9"))
    r = C.classify_and_act("x", classes=CLASSES)
    assert r.cls is None and r.escalate


def test_handler_exception_escalates(monkeypatch):
    monkeypatch.setattr(C, "call", lambda *a, **k: _Res("CLASS: bulk\nCONFIDENCE: 0.9"))
    def _boom(req):
        raise RuntimeError("fallo")
    r = C.classify_and_act("x", classes=CLASSES, handlers={"bulk": _boom})
    assert r.escalate and not r.handled and r.result["handler_error"] == "RuntimeError"
