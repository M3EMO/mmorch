"""Veredicto humano con un comando — la señal GOLD del flywheel.

Uso (desde el repo, con el venv):
  python scripts/veredicto.py listar
  python scripts/veredicto.py cand <id> dale|no      (candidata del roadmap)
  python scripts/veredicto.py card <id> dale|no      (tarjeta nota->proyecto)

dale candidata -> Archivadas estado=promovida + gist al roadmap.md + reward 1.0
no   candidata -> Archivadas estado=rechazada + reward 0.125
card           -> outcomes.record_verdict (aceptada/rechazada + reward al bandit)
"""

import io
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

from mmorch.fuel import parse_candidatos  # noqa: E402

CAND = ROOT / "vault" / "roadmaps" / "candidatos.md"
ROAD = ROOT / "vault" / "roadmaps" / "roadmap.md"


def listar() -> None:
    md = CAND.read_text(encoding="utf-8")
    print("== CANDIDATAS (veredicto: cand <id> dale|no) ==")
    for e in parse_candidatos(md):
        base = e["gist"].split(">>")[0].strip()
        print(f"[{e['id']}] ({e['lente']}, vence {e['vence']})\n    {base[:180]}\n")
    try:
        adj = json.loads((ROOT / "logs" / "adjudications.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return
    print("== TARJETAS nota->proyecto (veredicto: card <id> dale|no) ==")
    for proj, ms in sorted(adj.get("by_project", {}).items()):
        for m in ms:
            if m.get("status") == "pendiente":
                print(f"[{m['id']}]\n    {proj} <- {pathlib.Path(m['note_path']).name}"
                      f" (score {m['score']}): {m.get('justification', '')[:140]}\n")


def cand(cid: str, verdict: str) -> None:
    from mmorch.curation import verdict_candidata
    print(verdict_candidata(cid, verdict))


def card(pid: str, verdict: str) -> None:
    from mmorch.curation import verdict_card
    print(verdict_card(pid, verdict))


def main() -> None:
    if len(sys.argv) < 2 or sys.argv[1] == "listar":
        listar()
    elif sys.argv[1] == "cand" and len(sys.argv) == 4:
        cand(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == "card" and len(sys.argv) == 4:
        card(sys.argv[2], sys.argv[3])
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
