import atexit
import os
import shutil
import tempfile

# Antes de importar mmorch: los paths de estado se anclan a import-time (paths.py). Sin esto la suite escribia en los
# logs reales, el bandit de ruteo y el vault (auditoria 2026-10-26). Los subprocess heredan el env.
os.environ["MMORCH_HOME"] = tempfile.mkdtemp(prefix="mmorch-test-")
atexit.register(shutil.rmtree, os.environ["MMORCH_HOME"], True)

import pytest  # noqa: E402

from mmorch import providers  # noqa: E402


@pytest.fixture(autouse=True)
def _breakers_limpios():
    """El breaker por modelo es estado de modulo: 3 fallos simulados en un test lo abrian 60s y los tests de
    providers que corrian despues fallaban o no segun la velocidad de la suite (2026-10-14)."""
    providers._BREAKERS.clear()
