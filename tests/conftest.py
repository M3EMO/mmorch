import pytest

from mmorch import providers


@pytest.fixture(autouse=True)
def _breakers_limpios():
    """El breaker por modelo es estado de modulo: 3 fallos simulados en un test lo abrian 60s y los tests de
    providers que corrian despues fallaban o no segun la velocidad de la suite (2026-10-14)."""
    providers._BREAKERS.clear()
