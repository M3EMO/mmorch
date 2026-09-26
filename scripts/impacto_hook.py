"""Hook de impacto: PreToolUse en Claude Code (`hook`), postToolUse en Cursor (`cursor`).

Lógica en mmorch/impacto.py. Este script existe porque el venv no instala el paquete: agrega el
repo al sys.path, igual que los otros hooks de scripts/. Fail-open: ante cualquier problema,
silencio y exit 0.
"""

import sys
from pathlib import Path

_REPO = Path(__file__).resolve().parents[1]

if __name__ == "__main__":
    try:
        sys.path.insert(0, str(_REPO))
        from mmorch.impacto import main

        sys.exit(main(sys.argv[1:] or ["hook"]))
    except Exception:
        sys.exit(0)
