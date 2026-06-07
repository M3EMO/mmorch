"""vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian.

mmorch escribe hallazgos verificados como notas markdown con frontmatter, y los
relee. Es la capa de memoria semantica (distinta del memo cache content-hash):
aca viven hechos/decisiones/research curados, navegables por humano (Obsidian).
"""
from __future__ import annotations

import re
from pathlib import Path

VAULT = Path(__file__).resolve().parent.parent / "vault"


def _slug(s: str) -> str:
    s = re.sub(r"[^\w\s-]", "", s.lower()).strip()
    return re.sub(r"[\s_]+", "-", s)[:60] or "nota"


def write_note(folder: str, title: str, body: str, *, frontmatter: dict | None = None) -> Path:
    """Escribe una nota markdown con frontmatter YAML simple. Devuelve el path."""
    fm = {"title": title, **(frontmatter or {})}
    lines = ["---"]
    for k, v in fm.items():
        if isinstance(v, list):
            lines.append(f"{k}: [{', '.join(str(x) for x in v)}]")
        else:
            lines.append(f"{k}: {v}")
    lines.append("---\n")
    p = VAULT / folder / f"{_slug(title)}.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("\n".join(lines) + body.strip() + "\n", encoding="utf-8")
    return p


def read_notes(folder: str) -> list[dict]:
    """Lee notas de una carpeta. Devuelve [{path, title, frontmatter, body}]."""
    out = []
    d = VAULT / folder
    if not d.exists():
        return out
    for p in sorted(d.glob("*.md")):
        txt = p.read_text(encoding="utf-8")
        fm, body = _split_frontmatter(txt)
        out.append({"path": str(p), "title": fm.get("title", p.stem),
                    "frontmatter": fm, "body": body})
    return out


def _split_frontmatter(txt: str) -> tuple[dict, str]:
    if not txt.startswith("---"):
        return {}, txt
    parts = txt.split("---", 2)
    if len(parts) < 3:
        return {}, txt
    fm = {}
    for ln in parts[1].strip().splitlines():
        if ":" in ln:
            k, _, v = ln.partition(":")
            fm[k.strip()] = v.strip()
    return fm, parts[2].strip()
