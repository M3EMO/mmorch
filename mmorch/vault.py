"""vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian.

mmorch escribe hallazgos verificados como notas markdown con frontmatter, y los
relee. Es la capa de memoria semantica (distinta del memo cache content-hash):
aca viven hechos/decisiones/research curados, navegables por humano (Obsidian).
"""
from __future__ import annotations

import re
from datetime import date
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
    out: list = []
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


def log_op(op: str, title: str) -> None:
    """Log de operaciones parseable (patron Estudio): append-only en VAULT/log.md,
    una linea `## [YYYY-MM-DD] <op> | <titulo>` — `grep "^## \\["` es la interfaz
    de query. Ops: write | ingest | migrate | mine | note. Best-effort."""
    try:
        lp = VAULT / "log.md"
        header = "" if lp.exists() else (
            "# Log de operaciones del vault\n\n"
            "Append-only, parseable: `grep \"^## \\[\" log.md`. "
            "Ops: write | ingest | migrate | mine | note.\n\n")
        with lp.open("a", encoding="utf-8") as f:
            f.write(f"{header}## [{date.today().isoformat()}] {op} | {title}\n")
    except OSError:
        pass  # side-channel (auditoria): su fallo nunca rompe la operacion


def write_validated(title: str, body: str, *, project: str, folder: str = 'research',
                    frontmatter: dict | None = None, remember_fn=None,
                    enqueue_babel_fn=None) -> Path:
    """Escribe una nota validada, regenera el MOC y dispara callbacks opcionales."""
    if not title or not title.strip():
        raise ValueError("title must be non-empty")
    if not project or not project.strip():
        raise ValueError("project must be non-empty")

    fm = {
        "created": date.today().isoformat(),
        "tags": [project],
        **(frontmatter or {}),
    }
    if "tags" in fm and isinstance(fm["tags"], list) and project not in fm["tags"]:
        fm["tags"].append(project)

    p = write_note(folder, title, body, frontmatter=fm)
    regenerate_moc(project)
    log_op("write", f"{title} [{project}]")

    if remember_fn is not None:
        try:
            # gist TEXTUAL: es lo que remember() indexa — el recall encuentra el
            # gist y la sesion lee la nota completa por el path (decision 09).
            remember_fn(f"[{project}] {title} — nota del vault en {p}")
        except Exception:
            pass
    if enqueue_babel_fn is not None:
        try:
            enqueue_babel_fn(p)
        except Exception:
            pass  # side-channel (cola babel): su fallo nunca rompe el write
    return p


def regenerate_moc(project: str) -> Path:
    """Genera/actualiza el MOC del proyecto escaneando el vault."""
    if not project or not project.strip():
        raise ValueError("project must be non-empty")

    sections: dict[str, list[str]] = {}
    excluded_dirs = {"moc", "templates", "archive", ".obsidian"}

    for folder in sorted(VAULT.iterdir()):
        if not folder.is_dir() or folder.name in excluded_dirs:
            continue
        for p in sorted(folder.glob("*.md")):
            if p.name.endswith(".babel.md"):
                continue
            txt = p.read_text(encoding="utf-8")
            fm, _ = _split_frontmatter(txt)
            # tags viene como string "[a, b, c]" del frontmatter: parsear a lista
            # (membresia exacta — substring haria que 'ai' matchee 'ai-notes')
            tags = fm.get("tags", "").strip("[]").replace(",", " ").split()
            if project not in tags:
                continue
            # los frontmatter viejos traen comentarios inline ("applied   # ..."):
            # el MOC muestra solo el valor
            status = fm.get("status", "").split("#")[0].strip()
            confidence = fm.get("confidence", "").split("#")[0].strip()
            babel_ok = "babel OK" if (p.with_suffix(".babel.md")).exists() else ""
            parts = [f"- [[{p.stem}]]"]
            if status:
                parts.append(f"— {status}")
            if confidence:
                parts.append(f"· conf {confidence}")
            if babel_ok:
                parts.append(f"· {babel_ok}")
            sections.setdefault(folder.name, []).append(" ".join(parts))

    moc_dir = VAULT / "moc"
    moc_dir.mkdir(parents=True, exist_ok=True)
    moc_path = moc_dir / f"{_slug(project)}.md"

    lines = [f"# {project}", ""]
    for sec in sorted(sections):
        lines.append(f"## {sec}")
        lines.extend(sections[sec])
        lines.append("")
    moc_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
    return moc_path
