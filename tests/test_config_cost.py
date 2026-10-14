"""Invariantes de registry + costo. Puro, sin API."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest
from mmorch.config import REGISTRY, spec, DEFAULT_VERIFIER, CHECKABLE_VERIFIER
from mmorch.cost import cost_usd


def test_cost_math():
    # deepseek-chat: price_in 0.14, price_out 0.28 USD/1M
    assert cost_usd("deepseek-chat", 1_000_000, 0) == pytest.approx(0.14)
    assert cost_usd("deepseek-chat", 0, 1_000_000) == pytest.approx(0.28)
    assert cost_usd("deepseek-chat", 0, 0) == 0.0


def test_unknown_model_raises():
    with pytest.raises(KeyError):
        spec("nope")


def test_default_verifiers_reason():
    # GOAL 2026-10-08: "El verificador razona"; la familia es libre (sin cross-family desde 2026-10-07).
    # Medido 2026-09-07: con thinking apagado el verificador rechaza ~40% del trabajo correcto.
    for key in (DEFAULT_VERIFIER, CHECKABLE_VERIFIER):
        assert ("thinking", {"type": "disabled"}) not in spec(key).extra_body


def test_anthropic_not_a_node():
    # Opus es el orquestador, NUNCA un nodo externo -> no debe estar en REGISTRY.
    assert not any(s.family == "anthropic" for s in REGISTRY.values())
