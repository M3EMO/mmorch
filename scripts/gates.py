"""Gates estaticos en un solo comando: ruff + mypy + grep-gate de paths.

Uso: .venv/Scripts/python.exe scripts/gates.py
Sale 0 solo si los TRES pasan — el mismo criterio que el hook pre-commit,
corrible a mano antes de commitear o desde CI.
"""

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# El grep-gate vive como test (tests/test_paths.py) — se corre via pytest
# para no duplicar la allowlist en dos lugares.
GATES = [
    ("ruff", [sys.executable, "-m", "ruff", "check", "."]),
    ("mypy", [sys.executable, "-m", "mypy", "mmorch"]),
    ("paths-grep", [sys.executable, "-m", "pytest", "-q",
                    "tests/test_paths.py::test_gate_sin_anclas_de_estado_fuera_de_paths"]),
]


def main() -> int:
    fallos = []
    for nombre, cmd in GATES:
        r = subprocess.run(cmd, cwd=str(REPO))
        print(f"[gate] {nombre}: {'OK' if r.returncode == 0 else 'FAIL'}")
        if r.returncode != 0:
            fallos.append(nombre)
    if fallos:
        print(f"[gate] fallaron: {', '.join(fallos)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
