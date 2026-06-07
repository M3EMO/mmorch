"""bucket_rank: graduar set en tiers, paralelo, alineacion preservada en fallos."""
import sys, pathlib, importlib
from dataclasses import dataclass
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
B = importlib.import_module("mmorch.bucketrank")


@dataclass
class _Res:
    text: str
    cost_usd: float = 0.0


def test_empty():
    r = B.bucket_rank([], rubric="x")
    assert r.graded == [] and all(v == [] for v in r.by_tier.values())


def test_groups_by_tier(monkeypatch):
    # tier segun el contenido del item (S si 'bueno', D si no).
    def _c(model, messages, **kw):
        item = messages[-1]["content"]
        return _Res("razon...\nTIER: S" if "bueno" in item else "razon...\nTIER: D")
    monkeypatch.setattr(B, "call", _c)
    r = B.bucket_rank(["item bueno", "item malo", "otro bueno"], rubric="calidad")
    assert set(r.by_tier["S"]) == {"item bueno", "otro bueno"}
    assert r.by_tier["D"] == ["item malo"]


def test_unparseable_falls_to_lowest(monkeypatch):
    monkeypatch.setattr(B, "call", lambda *a, **k: _Res("sin tier aca"))
    r = B.bucket_rank(["x"], rubric="r", tiers=["A", "B", "C"])
    assert r.by_tier["C"] == ["x"]


def test_failure_keeps_item(monkeypatch):
    def _boom(*a, **k):
        raise RuntimeError("api down")
    monkeypatch.setattr(B, "call", _boom)
    r = B.bucket_rank(["a", "b"], rubric="r", tiers=["S", "D"])
    assert r.n_failed == 2 and r.by_tier["D"] == ["a", "b"]  # nada se pierde


def test_custom_tiers(monkeypatch):
    monkeypatch.setattr(B, "call", lambda *a, **k: _Res("TIER: alta"))
    r = B.bucket_rank(["x"], rubric="r", tiers=["alta", "media", "baja"])
    assert r.by_tier["alta"] == ["x"]
