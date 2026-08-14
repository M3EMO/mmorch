"""Hook SessionStart: inyecta (stdout) la mejor propuesta pendiente del proyecto del cwd.

Local puro (sin red, sin LLM), fail-open: ante cualquier problema, silencio y exit 0.
"""

import json
import sys
from pathlib import Path

_REPO = Path(__file__).resolve().parents[1]


def main() -> None:
    try:
        try:
            data = json.load(sys.stdin)
        except Exception:
            data = {}
        sys.path.insert(0, str(_REPO))
        from mmorch.proposals import pick_card

        projects = json.loads((_REPO / "projects.json").read_text(encoding="utf-8"))
        cwd = data.get("cwd") or str(Path.cwd())
        card = pick_card(cwd, projects, logs_dir=str(_REPO / "logs"))
        if card:
            print(card)
    except Exception:
        pass
    sys.exit(0)


if __name__ == "__main__":
    main()
