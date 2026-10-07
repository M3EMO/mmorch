"""Hook de impacto: PreToolUse en Claude Code (`hook`), postToolUse en Cursor (`cursor`).

Lógica en mmorch/impacto.py. Este script existe porque el venv no instala el paquete: agrega el
repo al sys.path, igual que los otros hooks de scripts/. Fail-open: ante cualquier problema,
silencio y exit 0.
"""

import sys
import types
from pathlib import Path

_REPO = Path(__file__).resolve().parents[1]

if __name__ == "__main__":
    try:
        # Paquete `mmorch` sin ejecutar su __init__: ese import carga providers y openai (~8 s)
        # y el hook corta a los 15 s. impacto e impacto_* usan solo la biblioteca estandar.
        pkg = types.ModuleType("mmorch")
        pkg.__path__ = [str(_REPO / "mmorch")]
        sys.modules["mmorch"] = pkg
        from mmorch.impacto import main

        sys.exit(main(sys.argv[1:] or ["hook"]))
    except Exception:
        sys.exit(0)
