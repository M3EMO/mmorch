"""Fuel module: candidate proposal lifecycle for roadmap loops."""

from __future__ import annotations

import os
import re
from datetime import date, timedelta
from typing import Callable, Optional

import mmorch.feedback

LENTES = ("deuda", "capacidad", "integracion", "notas-huerfanas")
MAX_CANDIDATAS = 5
EXPIRY_DAYS = 14
ARM_PREFIX = "propuesta:roadmap-"

_CAND_RE = re.compile(
    r"^-\s+\*\*cand-(?P<id>[^*]+)\*\*\s*\|\s*"
    r"fecha:\s*(?P<fecha>\S+)\s*\|\s*"
    r"vence:\s*(?P<vence>\S+)\s*\|\s*"
    r"lente:\s*(?P<lente>[^|]+?)\s*\|\s*"
    r"gist:\s*(?P<gist>.+?)"
    r"(?:\s*\|\s*estado:\s*(?P<estado>\S+))?\s*$"
)


def _parse_section(md_text: str, header: str) -> list[dict]:
    """Parse the bullets of one '## <header>' section."""
    entries = []
    in_section = False
    for line in md_text.splitlines():
        stripped = line.strip()
        if stripped.startswith(f"## {header}"):
            in_section = True
            continue
        if stripped.startswith("## ") and in_section:
            break
        if not in_section or not stripped.startswith("- **cand-"):
            continue
        m = _CAND_RE.match(stripped)
        if m:
            entries.append(
                {
                    "id": m.group("id").strip(),
                    "fecha": m.group("fecha").strip(),
                    "vence": m.group("vence").strip(),
                    "lente": m.group("lente").strip(),
                    "gist": m.group("gist").strip(),
                    "estado": (m.group("estado") or "pendiente").strip(),
                }
            )
    return entries


def parse_candidatos(md_text: str) -> list[dict]:
    """Parse '## Vigentes' section bullets starting with '- **cand-'."""
    return _parse_section(md_text, "Vigentes")


def parse_archivadas(md_text: str) -> list[dict]:
    """Parse '## Archivadas' section bullets."""
    return _parse_section(md_text, "Archivadas")


def render_candidatos(entries: list[dict], archived: list[dict]) -> str:
    """Render candidates markdown with fixed header, Vigentes and Archivadas."""
    lines = [
        "# Candidatas",
        "",
        "## Vigentes",
        "",
    ]
    for e in entries:
        lines.append(
            f"- **cand-{e['id']}** | fecha: {e['fecha']} | vence: {e['vence']} "
            f"| lente: {e['lente']} | gist: {e['gist']} | estado: {e['estado']}"
        )
    lines.extend(["", "## Archivadas", ""])
    for e in archived:
        lines.append(
            f"- **cand-{e['id']}** | fecha: {e['fecha']} | vence: {e['vence']} "
            f"| lente: {e['lente']} | gist: {e['gist']} | estado: {e['estado']}"
        )
    return "\n".join(lines) + "\n"


def has_new_fuel(since_ts: float, paths: list[str]) -> bool:
    """Check if any path has mtime newer than since_ts."""
    for p in paths:
        try:
            if os.path.getmtime(p) > since_ts:
                return True
        except OSError:
            continue
    return False


def generate_candidates(
    fuel_context,
    generator,
    verifier,
    *,
    candidatos_path: str,
    roadmap_path: str,
    today: str,
) -> dict:
    """Generate new candidate proposals per lente."""
    # Read existing candidates and roadmap
    try:
        with open(candidatos_path, "r", encoding="utf-8") as f:
            md_text = f.read()
    except FileNotFoundError:
        md_text = ""
    try:
        with open(roadmap_path, "r", encoding="utf-8") as f:
            roadmap_text = f.read()
    except FileNotFoundError:
        roadmap_text = ""

    existing = parse_candidatos(md_text)
    archived_entries = parse_archivadas(md_text)
    ya_visto = [e["gist"].lower().strip() for e in existing + archived_entries]
    if roadmap_text.strip():
        ya_visto.append(roadmap_text.lower())
    roadmap_lower = roadmap_text.lower()

    survivors = []
    for lente in LENTES:
        proposal = generator.propose(
            {"lente": lente, "context": fuel_context, "ya_visto": ya_visto}
        )
        gist = (proposal.get("gist") or "").strip()
        if not gist:
            continue
        refutation = verifier.refute({"lente": lente, "gist": gist})
        if refutation and refutation.get("refuted"):
            continue
        key = gist.lower().strip()
        # dedup contra candidatas (vigentes+archivadas) Y contra el roadmap curado
        if key in ya_visto or (roadmap_lower and key in roadmap_lower):
            continue
        ya_visto.append(key)
        survivors.append({"lente": lente, "gist": gist})
        if len(survivors) >= MAX_CANDIDATAS:
            break

    # Build new entries; NN sigue contando las ya creadas HOY (sin colision al re-correr)
    same_day = sum(1 for e in existing + archived_entries if e["id"].startswith(today))
    new_entries = []
    vence_date = date.fromisoformat(today) + timedelta(days=EXPIRY_DAYS)
    vence_str = vence_date.isoformat()
    for i, s in enumerate(survivors, start=same_day + 1):
        new_entries.append(
            {
                "id": f"{today}-{i:02d}",
                "fecha": today,
                "vence": vence_str,
                "lente": s["lente"],
                "gist": s["gist"],
                "estado": "pendiente",
            }
        )

    # Write file: combine existing vigentes + new + archived
    all_vigentes = existing + new_entries
    all_archived = archived_entries
    output = render_candidatos(all_vigentes, all_archived)
    with open(candidatos_path, "w", encoding="utf-8") as f:
        f.write(output)

    return {"nuevas": len(new_entries)}


def expire_candidates(
    *,
    candidatos_path: str,
    today: str,
    record_fn: Optional[Callable] = None,
) -> dict:
    """Move expired candidates to Archivadas."""
    if record_fn is None:
        record_fn = mmorch.feedback.record_outcome

    try:
        with open(candidatos_path, "r", encoding="utf-8") as f:
            md_text = f.read()
    except FileNotFoundError:
        return {"expired": 0}

    entries = parse_candidatos(md_text)
    archived_entries = parse_archivadas(md_text)

    today_date = date.fromisoformat(today)
    expired = []
    remaining = []
    for e in entries:
        try:
            vence_date = date.fromisoformat(e["vence"])
        except ValueError:
            remaining.append(e)
            continue
        if vence_date < today_date:
            e["estado"] = "expirada"
            expired.append(e)
            record_fn(
                f"{ARM_PREFIX}{e['lente']}",
                0.2,
                pattern="loop_propuestas",
                source="soft_reject",
            )
        else:
            remaining.append(e)

    all_archived = archived_entries + expired
    output = render_candidatos(remaining, all_archived)
    with open(candidatos_path, "w", encoding="utf-8") as f:
        f.write(output)

    return {"expired": len(expired)}


def detect_promotions(
    *,
    candidatos_path: str,
    roadmap_path: str,
    record_fn: Optional[Callable] = None,
) -> dict:
    """Move promoted candidates to Archivadas."""
    if record_fn is None:
        record_fn = mmorch.feedback.record_outcome

    try:
        with open(candidatos_path, "r", encoding="utf-8") as f:
            md_text = f.read()
    except FileNotFoundError:
        return {"promoted": 0}

    try:
        with open(roadmap_path, "r", encoding="utf-8") as f:
            roadmap_text = f.read()
    except FileNotFoundError:
        roadmap_text = ""

    entries = parse_candidatos(md_text)
    archived_entries = parse_archivadas(md_text)

    roadmap_lower = roadmap_text.lower()
    promoted = []
    remaining = []
    for e in entries:
        gist_key = e["gist"].lower().strip()[:60]
        if gist_key in roadmap_lower:
            e["estado"] = "promovida"
            promoted.append(e)
            record_fn(
                f"{ARM_PREFIX}{e['lente']}",
                1.0,
                pattern="loop_propuestas",
                source="roadmap_promotion",
            )
        else:
            remaining.append(e)

    all_archived = archived_entries + promoted
    output = render_candidatos(remaining, all_archived)
    with open(candidatos_path, "w", encoding="utf-8") as f:
        f.write(output)

    return {"promoted": len(promoted)}
