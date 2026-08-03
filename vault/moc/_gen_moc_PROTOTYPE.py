# PROTOTYPE — throwaway (ticket 04 wayfinder vault-global). Correr:
#   python vault/moc/_gen_moc_PROTOTYPE.py
# Genera vault/moc/<proyecto>.md desde los frontmatter reales. Sin polish a proposito.
from __future__ import annotations

from pathlib import Path

VAULT = Path(__file__).resolve().parent.parent
PROJECTS = {"mmorch", "lotus", "portfolio", "experimentotrabajo"}
SKIP_DIRS = {"moc", "templates", "archive", ".obsidian"}


def fm(text: str) -> dict:
    if not text.startswith("---"):
        return {}
    out = {}
    for ln in text.split("---", 2)[1].splitlines():
        if ":" in ln:
            k, _, v = ln.partition(":")
            out[k.strip()] = v.strip()
    return out


notes: dict[str, list[dict]] = {}
for p in VAULT.rglob("*.md"):
    rel = p.relative_to(VAULT)
    if rel.parts[0] in SKIP_DIRS or p.name.endswith(".babel.md"):
        continue
    f = fm(p.read_text(encoding="utf-8"))
    tags = f.get("tags", "").strip("[]").replace(",", " ").lower().split()
    proj = next((t for t in tags if t in PROJECTS), "sin-proyecto")
    notes.setdefault(proj, []).append({
        "name": p.stem, "folder": rel.parts[0] if len(rel.parts) > 1 else "raiz",
        "status": f.get("status", "?").split("#")[0].strip(),
        "conf": f.get("confidence", "").split("#")[0].strip(),
        "babel": p.with_suffix(".babel.md").exists(),
    })

for proj, ns in notes.items():
    lines = [f"# MOC — {proj}", "",
             f"_{len(ns)} notas · generado por `_gen_moc_PROTOTYPE.py` — no editar a mano_", ""]
    for folder in sorted({n["folder"] for n in ns}):
        lines.append(f"## {folder}")
        for n in sorted((x for x in ns if x["folder"] == folder), key=lambda x: x["name"]):
            extra = " · ".join(x for x in [
                n["status"] if n["status"] not in ("?", "") else "",
                f"conf {n['conf']}" if n["conf"] else "",
                "babel ✓" if n["babel"] else ""] if x)
            lines.append(f"- [[{n['name']}]]" + (f" — {extra}" if extra else ""))
        lines.append("")
    out = VAULT / "moc" / f"{proj}.md"
    out.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{out.name}: {len(ns)} notas")
