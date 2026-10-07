"""vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian.

mmorch escribe hallazgos verificados como notas markdown con frontmatter, y los
relee. Es la capa de memoria semantica (distinta del memo cache content-hash):
aca viven hechos/decisiones/research curados, navegables por humano (Obsidian).
"""
from __future__ import annotations

import hashlib
import json
import re
from datetime import date
from pathlib import Path

from .paths import home

VAULT = home() / "vault"


def _slug(s: str) -> str:
    s = re.sub(r"[^\w\s-]", "", s.lower()).strip()
    return re.sub(r"[\s_]+", "-", s)[:60] or "nota"


def _safe_folder(folder: str) -> Path:
    """Clamp del param `folder` (expuesto por la tool MCP mmorch_vault_write): sin esto,
    '../..' escribia FUERA del vault y hasta fuera de MMORCH_HOME (path traversal, W6).
    Lee VAULT del modulo en runtime pa respetar el monkeypatch de los tests."""
    base = Path(VAULT).resolve()
    d = (base / str(folder)).resolve()
    if d != base and base not in d.parents:
        raise ValueError(f"folder invalido (escapa del vault): {folder!r}")
    return d


def _yaml_str(v, *, flow: bool = False) -> str:
    """Escalar YAML: plano si yaml lo lee igual, entre comillas dobles si no (un string
    JSON es YAML valido). Medido 2026-10-02: 'title: a: b' y una URL con '?' dentro de
    [..] dejaban el frontmatter ilegible, y adjudicate lo borraba entero."""
    s = str(v)
    risky = (not s or s != s.strip() or s.startswith(tuple("-?:,[]{}#&*!|>'\"%@`"))
             or ": " in s or " #" in s or s.endswith(":")
             or (flow and any(c in s for c in ",[]{}:?#")))
    return json.dumps(s, ensure_ascii=False) if risky else s


def write_note(folder: str, title: str, body: str, *, frontmatter: dict | None = None) -> Path:
    """Escribe una nota markdown con frontmatter YAML simple. Devuelve el path.

    _slug trunca a 60 chars (colisiones deterministas) y dos titulos distintos pueden
    mapear al mismo path. Si el path ya existe con OTRO contenido, no lo pisamos en
    silencio (la nota vieja seria irrecuperable sin backup/commit): buscamos el proximo
    sufijo libre `-2`, `-3`, ... Mismo titulo + mismo contenido (idempotencia real) SI
    reusa el path — no es una colision, es un re-write."""
    fm = {"title": title, **(frontmatter or {})}
    lines = ["---"]
    for k, v in fm.items():
        if isinstance(v, list):
            lines.append(f"{k}: [{', '.join(_yaml_str(x, flow=True) for x in v)}]")
        else:
            lines.append(f"{k}: {_yaml_str(v)}")
    lines.append("---\n")
    text = "\n".join(lines) + body.strip() + "\n"

    slug = _slug(title)
    d = _safe_folder(folder)
    p = d / f"{slug}.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    if p.exists() and p.read_text(encoding="utf-8") != text:
        n = 2
        while (cand := d / f"{slug}-{n}.md").exists() and cand.read_text(encoding="utf-8") != text:
            n += 1
        p = cand
        log_op("overwrite_avoided", f"{title} -> {p.name} (colision de slug)")
    p.write_text(text, encoding="utf-8")
    return p


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


def _read_frontmatter_only(p: Path) -> dict:
    """Lee SOLO el bloque frontmatter (hasta el 2do '---'), sin cargar el body entero.
    regenerate_moc solo necesita tags/applies_to/status/confidence del frontmatter -> evita
    read_text() de todo el .md (O(vault) por write cuando el vault crece)."""
    lines: list[str] = []
    with p.open(encoding="utf-8") as fh:
        first = fh.readline()
        if first.strip() != "---":
            return {}
        for ln in fh:
            if ln.strip() == "---":
                break
            lines.append(ln)
    fm: dict = {}
    open_key = None  # clave con valor vacio: puede abrir una lista en bloque ("- item")
    for ln in lines:
        item = re.match(r"\s*-\s+(.*)", ln)
        if item and open_key is not None:
            fm[open_key] = [*(fm[open_key] or []), item.group(1).strip()]
            continue
        if ":" in ln:
            k, _, v = ln.partition(":")
            fm[k.strip()] = _unquote(v.strip())
            open_key = k.strip() if not v.strip() else None
    return fm


def _unquote(v: str) -> str:
    """Inverso de _yaml_str para el lector por lineas: '"a: b"' -> 'a: b'."""
    if len(v) >= 2 and v[0] == v[-1] == '"':
        try:
            return json.loads(v)
        except ValueError:
            return v[1:-1]
    return v


def _as_list(v: str | list) -> list[str]:
    """Lista del frontmatter en cualquiera de sus formas: en bloque ya llega como list,
    inline llega como string "[a, b]". Items exactos (membresia por igualdad, nunca
    substring: 'ai' no debe matchear 'ai-notes')."""
    items = v if isinstance(v, list) else v.strip("[]").split(",")
    return [x.strip().strip("'\"") for x in items if x.strip()]


def log_op(op: str, title: str, *, base: Path | None = None) -> None:
    """Log de operaciones parseable (patron Estudio): append-only en VAULT/log.md,
    una linea `## [YYYY-MM-DD] <op> | <titulo>` — `grep "^## \\["` es la interfaz
    de query. Ops: write | ingest | migrate | mine | note. Best-effort."""
    try:
        lp = (base or VAULT) / "log.md"
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


def write_research_note(title: str, body: str, *, project: str, folder: str = "research",
                        status: str = "seed", confidence: str = "", sources: str = "",
                        tags: str = "") -> tuple[Path, Path]:
    """Puerta de ALTO nivel para callers de borde (MCP/CLI): arma el frontmatter desde
    strings CSV, escribe la nota validada, puentea un gist a memoria (scope global) y
    dispara babel ingest async. Antes esta orquestacion vivia en el wrapper MCP (W5.1:
    la logica va en la libreria, el wrapper solo adapta tipos). Devuelve (nota, moc)."""
    import threading

    fm: dict = {"status": status or "seed"}
    if confidence:
        fm["confidence"] = confidence
    if sources:
        fm["sources"] = [s.strip() for s in sources.split(",") if s.strip()]
    extra = [t.strip() for t in (tags or "").split(",") if t.strip()]
    fm["tags"] = ["research", project] + [t for t in extra if t != project]

    def _bridge(gist: str) -> None:
        # decision 09: gist textual a duckdb scope global; el recall existente lo
        # encuentra y la sesion lee la nota completa por path. Import lazy: vault no
        # debe depender duro de memory (y remember gasta una call barata de destilado).
        from .memory import remember
        remember("global", gist, kind="vault_note")

    def _babel_async(path: Path) -> None:
        # decision 03 pedia cola del server; thread daemon = mismo efecto async sin
        # endpoint nuevo. El nightly barre notas sin babel: perder el thread no pierde nada.
        def _run() -> None:
            try:
                from .babel import ingest
                ingest(path, folder=folder)
            except Exception:
                pass  # side-channel: el gate/nightly deciden, jamas romper el write
        threading.Thread(target=_run, daemon=True).start()

    p = write_validated(title, body, project=project, folder=folder,
                        frontmatter=fm, remember_fn=_bridge,
                        enqueue_babel_fn=_babel_async)
    return p, _moc_path(project)


def _moc_path(project: str) -> Path:
    """Path del MOC de un proyecto. _slug pierde informacion ('.claude' y 'Claude' dan
    'claude', y Windows no distingue mayusculas): un nombre que no es su propio slug
    lleva un sufijo con hash del nombre exacto, asi dos proyectos nunca comparten MOC.
    Los nombres que ya son slug (orchestration, mmorch) conservan su archivo."""
    slug = _slug(project)
    if slug != project:
        slug = f"{slug}-{hashlib.sha256(project.encode('utf-8')).hexdigest()[:8]}"
    return VAULT / "moc" / f"{slug}.md"


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
            fm = _read_frontmatter_only(p)
            # una nota declara su proyecto en `tags` o en `applies_to` (notas viejas,
            # 2026-10-01: sin esto desaparecian del MOC en cada regeneracion)
            members = _as_list(fm.get("tags", "")) + _as_list(fm.get("applies_to", ""))
            if project not in members:
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

    moc_path = _moc_path(project)
    moc_path.parent.mkdir(parents=True, exist_ok=True)

    lines = [f"# {project}", ""]
    for sec in sorted(sections):
        lines.append(f"## {sec}")
        lines.extend(sections[sec])
        lines.append("")
    moc_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
    return moc_path
