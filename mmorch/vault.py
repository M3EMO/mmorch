"""vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian.

mmorch escribe hallazgos verificados como notas markdown con frontmatter, y los
relee. Es la capa de memoria semantica (distinta del memo cache content-hash):
aca viven hechos/decisiones/research curados, navegables por humano (Obsidian).
"""
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from datetime import date
from pathlib import Path

from . import config
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
                    supersedes: list[str] | None = None,
                    code: list[str] | None = None) -> Path:
    """Escribe una nota validada, regenera el MOC y dispara callbacks opcionales.
    `supersedes` (stems de notas vivas): se validan TODAS antes de escribir; despues la
    nueva guarda `supersedes` y las viejas se retiran a archive/ (ver _retire)."""
    if not title or not title.strip():
        raise ValueError("title must be non-empty")
    if not project or not project.strip():
        raise ValueError("project must be non-empty")
    _check_code(code)
    olds = _plan_supersedes(supersedes)

    fm = {
        "created": date.today().isoformat(),
        "tags": [project],
        **(frontmatter or {}),
    }
    if "tags" in fm and isinstance(fm["tags"], list) and project not in fm["tags"]:
        fm["tags"].append(project)
    if olds:
        fm["supersedes"] = [o.stem for o in olds]
    if code:
        fm["code"] = list(code)

    old_tags = {project}
    for o in olds:
        old_tags.update(_members(_read_frontmatter_only(o)))
    p = write_note(folder, title, body, frontmatter=fm)
    for o in olds:
        _retire(o, p.stem)
    # el MOC del proyecto siempre; el de otro tag de la vieja solo si ya existe
    for t in sorted(old_tags):
        if t == project or _moc_path(t).exists():
            regenerate_moc(t)
    log_op("write", f"{title} [{project}]")

    if remember_fn is not None:
        try:
            # gist TEXTUAL: es lo que remember() indexa — el recall encuentra el
            # gist y la sesion lee la nota completa por el path (decision 09).
            remember_fn(f"[{project}] {title} — nota del vault en {p}")
        except Exception:
            pass
    return p


def _check_code(code: list[str] | None) -> None:
    """`code` ata la nota a paths del repo; llega del MCP, asi que se valida antes de escribir."""
    if not code:
        return
    if "code" not in config.VAULT_MODULES:
        raise ValueError("modulo 'code' apagado (config.VAULT_MODULES)")
    for c in code:
        if (not c.strip() or not c.isprintable() or c.startswith(("/", "-", ":")) or "\\" in c
                or ".." in c.split("/")):
            raise ValueError(f"code invalido (path relativo al repo, sin '..'): {c!r}")


def _a_revisar(fm: dict) -> bool:
    """Marca 'a revisar': un commit en un path de `code` mas nuevo (estricto, P3) que
    `reviewed` o, si falta, que `created`. La marca no oculta la nota ni la reemplaza."""
    paths, ref = _as_list(fm.get("code", "")), fm.get("reviewed") or fm.get("created")
    if not paths or not ref or "code" not in config.VAULT_MODULES:
        return False
    for c in paths:
        try:
            r = subprocess.run(["git", "-C", str(VAULT), "log", "-1", "--format=%cs",
                                "--", f":(top){c}"], capture_output=True, text=True, timeout=5)
        except (OSError, subprocess.SubprocessError):
            continue  # side-channel: sin git no hay marca, nunca error
        if r.returncode == 0 and r.stdout.strip() > ref:
            return True
    return False


def note_a_revisar(path: str | Path) -> bool:
    """`_a_revisar` sobre una nota por su path (la usa el recall)."""
    p = Path(path)
    return p.is_file() and _a_revisar(_read_frontmatter_only(p))


_NO_VIVAS = {"moc", "templates", "archive", ".obsidian"}


def _live_folders() -> list[Path]:
    if not VAULT.is_dir():
        return []
    return [f for f in sorted(VAULT.iterdir()) if f.is_dir() and f.name not in _NO_VIVAS]


def _members(fm: dict) -> list[str]:
    """Proyectos de una nota: `tags` o `applies_to` (notas viejas)."""
    return _as_list(fm.get("tags", "")) + _as_list(fm.get("applies_to", ""))


def _resolve_note(name: str) -> Path:
    """Stem de `[[stem]]` -> path de la nota viva. El nombre llega del MCP: nada fuera
    del vault se resuelve. 0 o >1 coincidencias dan ValueError."""
    if not name or name in (".", "..") or Path(name).name != name or "\\" in name:
        raise ValueError(f"supersedes invalido: {name!r}")
    hits = [h for f in _live_folders()
            if (h := f / f"{name}.md").exists() and not h.is_symlink()]
    if len(hits) != 1:
        raise ValueError(f"supersedes {name!r}: {len(hits)} notas vivas (se esperaba 1)")
    return hits[0]


def _successor(fm: dict) -> str:
    """`superseded_by` del frontmatter (escalar o lista en bloque: vale el primero), sin [[ ]]."""
    return (_as_list(fm.get("superseded_by", "")) or [""])[0].strip("[] ")


def _existe(name: str) -> bool:
    """La nota `name` existe viva o en archive/ (nombre ya validado contra traversal)."""
    try:
        _resolve_note(name)
        return True
    except ValueError:
        return bool(name) and Path(name).name == name and (VAULT / "archive" / f"{name}.md").exists()


def _plan_supersedes(names: list[str] | None) -> list[Path]:
    """Valida todo antes de escribir: modulo prendido, nombres resolubles, sin choque en archive/."""
    if not names:
        return []
    if "supersedes" not in config.VAULT_MODULES:
        raise ValueError("modulo 'supersedes' apagado (config.VAULT_MODULES)")
    olds = [_resolve_note(n) for n in dict.fromkeys(names)]
    for o in olds:
        if (VAULT / "archive" / o.name).exists():
            raise ValueError(f"archive/{o.name} ya existe: no se pisa")
    return olds


def _retire(p: Path, successor: str | None) -> Path:
    """Mueve la nota a archive/ y, con sucesora, inserta `superseded_by` antes del cierre
    del frontmatter (sin reserializar: las listas en bloque sobreviven) y la linea
    "Reemplazada por [[x]]" al tope del cuerpo. Idempotente. successor=None solo mueve."""
    dest = VAULT / "archive" / p.name
    if p != dest and p.exists():
        if dest.exists():
            raise FileExistsError(f"archive/{p.name} ya existe: no se pisa")
        dest.parent.mkdir(parents=True, exist_ok=True)
        p.rename(dest)
    if successor and dest.exists():
        lines = dest.read_text(encoding="utf-8").split("\n")
        close = next((i for i, ln in enumerate(lines[1:], 1) if ln.strip() == "---"), None) \
            if lines and lines[0].strip() == "---" else None
        if close is None:
            lines = ["---", "---", *lines]
            close = 1
        if not any(ln.startswith("superseded_by:") for ln in lines[1:close]):
            lines.insert(close, f"superseded_by: {_yaml_str(successor)}")
            close += 1
        aviso = f"Reemplazada por [[{successor}]]"
        if aviso not in lines[close + 1:close + 3]:
            lines[close + 1:close + 1] = [aviso, ""]
        dest.write_text("\n".join(lines), encoding="utf-8")
    return dest


def _complete_by_hand(project: str) -> set[str]:
    """Completa la mitad que falta de una supersesion editada a mano (solo notas del
    proyecto). Un `supersedes` o `superseded_by` roto se loguea y se saltea: nunca rompe
    la regeneracion. Devuelve los proyectos de las viejas retiradas (sus MOC quedan viejos)."""
    tags: set[str] = set()
    if "supersedes" not in config.VAULT_MODULES:
        return tags
    for folder in _live_folders():
        for p in sorted(folder.glob("*.md")):
            if not p.exists() or p.is_symlink():  # la retiro una pasada anterior de este bucle
                continue
            fm = _read_frontmatter_only(p)
            if project not in _members(fm):
                continue
            try:
                for name in _as_list(fm.get("supersedes", "")):
                    if (VAULT / "archive" / f"{name}.md").exists():
                        continue
                    try:
                        old = _resolve_note(name)
                        tags.update(_members(_read_frontmatter_only(old)))
                        _retire(old, p.stem)
                    except ValueError as e:
                        log_op("supersedes_roto", f"{p.stem}: {e}")
                sb = _successor(fm)
                if sb and not _existe(sb):
                    log_op("supersedes_roto", f"{p.stem}: superseded_by {sb!r} no existe")
                elif sb:
                    _retire(p, sb)
            except OSError as e:
                log_op("supersedes_roto", f"{p.stem}: {e}")
    return tags


def write_research_note(title: str, body: str, *, project: str, folder: str = "research",
                        status: str = "seed", confidence: str = "", sources: str = "",
                        tags: str = "", supersedes: str = "", code: str = "") -> tuple[Path, Path]:
    """Puerta de ALTO nivel para callers de borde (MCP/CLI): arma el frontmatter desde
    strings CSV (`supersedes` = stems de notas a reemplazar, `code` = paths del repo), escribe la nota validada y puentea un gist a memoria (scope global).
    Babel se borro el 2026-11-26: nadie leyo un .babel.md en un mes y babel mas el
    lexicon costaba 96% del original. Devuelve (nota, moc)."""
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

    p = write_validated(title, body, project=project, folder=folder,
                        frontmatter=fm, remember_fn=_bridge,
                        supersedes=[s.strip() for s in supersedes.split(",") if s.strip()],
                        code=[c.strip() for c in code.split(",") if c.strip()])
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


def regenerate_moc(project: str, *, _completar: bool = True) -> Path:
    """Genera/actualiza el MOC del proyecto escaneando el vault."""
    if not project or not project.strip():
        raise ValueError("project must be non-empty")

    otros = _complete_by_hand(project) if _completar else set()
    sections: dict[str, list[str]] = {}

    for folder in _live_folders():
        for p in sorted(folder.glob("*.md")):
            fm = _read_frontmatter_only(p)
            # una nota declara su proyecto en `tags` o en `applies_to` (notas viejas,
            # 2026-10-01: sin esto desaparecian del MOC en cada regeneracion)
            if project not in _members(fm):
                continue
            # los frontmatter viejos traen comentarios inline ("applied   # ..."):
            # el MOC muestra solo el valor
            status = fm.get("status", "").split("#")[0].strip()
            confidence = fm.get("confidence", "").split("#")[0].strip()
            parts = [f"- [[{p.stem}]]"]
            if status:
                parts.append(f"— {status}")
            if confidence:
                parts.append(f"· conf {confidence}")
            if _a_revisar(fm):
                parts.append("· a revisar")
            sections.setdefault(folder.name, []).append(" ".join(parts))

    # archive/ solo arma las dos secciones finales (A3): con sucesora = Reemplazadas
    for p in sorted((VAULT / "archive").glob("*.md")):
        fm = _read_frontmatter_only(p)
        if project not in _members(fm):
            continue
        sb = _successor(fm)
        if sb:
            sections.setdefault("Reemplazadas", []).append(f"- [[{p.stem}]] → [[{sb}]]")
        else:
            sections.setdefault("Archivadas", []).append(f"- [[{p.stem}]]")

    moc_path = _moc_path(project)
    moc_path.parent.mkdir(parents=True, exist_ok=True)

    lines = [f"# {project}", ""]
    # las dos secciones de archive/ van al final, en este orden
    orden = [x for x in sorted(sections) if x not in ("Reemplazadas", "Archivadas")]
    orden += [x for x in ("Reemplazadas", "Archivadas") if x in sections]
    for sec in orden:
        lines.append(f"## {sec}")
        lines.extend(sections[sec])
        lines.append("")
    moc_path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
    # MOC de otros proyectos con viejas retiradas a mano: solo los que ya existen y sin
    # volver a completar (evita la recursion)
    for t in sorted(otros - {project}):
        if _moc_path(t).exists():
            regenerate_moc(t, _completar=False)
    return moc_path


def is_retired(path: str | Path) -> bool:
    """Una nota esta retirada si su path original falta y existe `archive/<nombre>`."""
    p = Path(path)
    return not p.exists() and (VAULT / "archive" / p.name).exists()
