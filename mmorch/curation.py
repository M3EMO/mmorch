"""Curacion humana de propuestas — logica compartida entre scripts/veredicto.py,
el endpoint /verdict del server y (futuro) Lotus.

dale candidata -> Archivadas estado=promovida + gist al roadmap.md + reward 1.0
no   candidata -> Archivadas estado=rechazada + reward 0.125
card           -> outcomes.record_verdict (dedup por status, reward al bandit)
"""

from __future__ import annotations

import json
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def pending(*, root: Path = ROOT) -> dict:
    """Todo lo que espera veredicto humano: candidatas vigentes + tarjetas."""
    from mmorch.fuel import parse_candidatos
    cands = []
    try:
        md = (root / "vault" / "roadmaps" / "candidatos.md").read_text(encoding="utf-8")
        for e in parse_candidatos(md):
            cands.append({"id": e["id"], "lente": e["lente"], "vence": e["vence"],
                          "gist": e["gist"].split(">>")[0].strip(),
                          "maduraciones": e["gist"].count(">>")})
    except OSError:
        pass
    cards = []
    try:
        adj = json.loads((root / "logs" / "adjudications.json")
                         .read_text(encoding="utf-8"))
        for proj, ms in (adj.get("by_project") or {}).items():
            for m in ms:
                if m.get("status") == "pendiente":
                    cards.append({"id": m["id"], "project": proj,
                                  "note": Path(m["note_path"]).name,
                                  "score": m["score"],
                                  "justification": m.get("justification", ""),
                                  "card": m.get("card", "")})
    except (OSError, json.JSONDecodeError):
        pass
    return {"candidatas": cands, "cards": cards}


def verdict_candidata(cid: str, verdict: str, *, root: Path = ROOT) -> dict:
    from mmorch.feedback import record_outcome
    from mmorch.fuel import ARM_PREFIX, parse_archivadas, parse_candidatos, render_candidatos
    if verdict not in ("dale", "no"):
        raise ValueError(f"verdict invalido: {verdict}")
    cand_path = root / "vault" / "roadmaps" / "candidatos.md"
    road_path = root / "vault" / "roadmaps" / "roadmap.md"
    md = cand_path.read_text(encoding="utf-8")
    vig, arch = parse_candidatos(md), parse_archivadas(md)
    target = next((e for e in vig if e["id"] == cid), None)
    if target is None:
        return {"ok": False, "error": f"candidata {cid} no encontrada"}
    vig.remove(target)
    if verdict == "dale":
        target["estado"], reward = "promovida", 1.0
        base = target["gist"].split(">>")[0].strip()
        road = (road_path.read_text(encoding="utf-8") if road_path.exists()
                else "# Roadmap\n\n## Direcciones\n")
        road_path.write_text(
            road.rstrip() + f"\n- {base}  <!-- cand-{cid}, dale "
            f"{time.strftime('%Y-%m-%d')} -->\n", encoding="utf-8")
    else:
        target["estado"], reward = "rechazada", 0.125
    arch.append(target)
    cand_path.write_text(render_candidatos(vig, arch), encoding="utf-8")
    record_outcome(f"{ARM_PREFIX}{target['lente']}", reward,
                   pattern="loop_propuestas", source="verdict")
    return {"ok": True, "id": cid, "estado": target["estado"], "reward": reward}


def verdict_card(pid: str, verdict: str, *, root: Path = ROOT) -> dict:
    from mmorch.outcomes import record_verdict
    r = record_verdict(pid, verdict, logs_dir=str(root / "logs"))
    return {"ok": bool(r.get("recorded")), **r}
