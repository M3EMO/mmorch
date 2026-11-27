"""vault_seeds — A4: archiva a mano las seeds que nadie leyo por nombre en 60 dias."""
from __future__ import annotations

import argparse
import json
import shlex
import sys
from datetime import date, datetime, timedelta
from pathlib import Path

from . import vault

DIAS = 60
_GLOB = set("*?[")
_LECTORES = {"cat", "sed", "grep"}
_NO_VIVAS = {"moc", "templates", "archive", ".obsidian"}


class CoberturaInsuficiente(RuntimeError):
    """Los transcripts cubren menos de 60 dias: sin evidencia suficiente para archivar."""


def _md(token: str) -> str | None:
    """Basename si el token nombra un archivo .md exacto (sin glob), si no None."""
    if token.endswith(".md") and not _GLOB & set(token):
        return Path(token).name
    return None


def _comandos(cmd: str) -> list[list[str]]:
    """Segmentos de un comando de shell, cortados en | && ; etc. Tolerante a comillas rotas."""
    try:
        lex = shlex.shlex(cmd, posix=True, punctuation_chars=True)
        lex.whitespace_split = True
        toks = list(lex)
    except ValueError:
        toks = cmd.split()  # shortcut: sin cortar por operadores, solo se pierde precision
    segs: list[list[str]] = [[]]
    for t in toks:
        if t and set(t) <= set("|&;()<>"):
            segs.append([])
        else:
            segs[-1].append(t)
    return [s for s in segs if s]


def _eventos(transcripts: Path):
    """(fecha, nombre_tool, input) por cada bloque tool_use de todos los JSONL."""
    for f in sorted(Path(transcripts).rglob("*.jsonl")):
        with f.open(encoding="utf-8", errors="replace") as fh:
            for ln in fh:
                try:
                    ev = json.loads(ln)
                    ts = datetime.fromisoformat(str(ev["timestamp"]).replace("Z", "+00:00")).date()
                except (ValueError, KeyError, TypeError):
                    continue
                content = (ev.get("message") or {}).get("content") if isinstance(ev, dict) else None
                blocks = content if isinstance(content, list) else []
                yielded = False
                for b in blocks:
                    if isinstance(b, dict) and b.get("type") == "tool_use":
                        yielded = True
                        yield ts, b.get("name"), b.get("input") or {}
                if not yielded:
                    yield ts, None, {}


def lecturas(transcripts: Path, desde: date) -> set[str]:
    """Basenames .md leidos por nombre exacto desde `desde`: Read, Grep con path a un
    archivo, y Bash cat/sed/grep. Un glob, una regex o un directorio no cuentan."""
    out: set[str] = set()
    for ts, name, inp in _eventos(transcripts):
        if ts < desde or not isinstance(inp, dict):
            continue
        if name == "Read":
            n = _md(str(inp.get("file_path", "")))
            if n:
                out.add(n)
        elif name == "Grep":
            n = _md(str(inp.get("path", "")))
            if n:
                out.add(n)
        elif name == "Bash":
            for seg in _comandos(str(inp.get("command", ""))):
                if seg[0] in _LECTORES:
                    out.update(n for t in seg[1:] if (n := _md(t)))
    return out


def candidatas(vault_dir: Path, transcripts: Path, hoy: date) -> list[Path]:
    corte = hoy - timedelta(days=DIAS)
    fechas = [ts for ts, _, _ in _eventos(transcripts)]
    if not fechas:
        raise CoberturaInsuficiente(f"sin transcripts legibles en {transcripts}")
    if min(fechas) > corte:
        raise CoberturaInsuficiente(
            f"los transcripts empiezan el {min(fechas)}: cubren menos de {DIAS} dias. "
            f"El comando puede correr desde {min(fechas) + timedelta(days=DIAS)}")
    leidas = lecturas(transcripts, corte)
    out = []
    for folder in sorted(vault_dir.iterdir()):
        if not folder.is_dir() or folder.name in _NO_VIVAS:
            continue
        for p in sorted(folder.glob("*.md")):
            fm = vault._read_frontmatter_only(p)
            if fm.get("status", "").split("#")[0].strip() != "seed":
                continue
            try:
                created = date.fromisoformat(fm.get("created", ""))
            except ValueError:
                continue
            if p.is_symlink():
                continue
            if created < corte and p.name not in leidas:
                out.append(p)
    return out


def main(argv: list[str] | None = None, hoy: date | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m mmorch.vault_seeds", description=__doc__)
    ap.add_argument("--aplicar", action="store_true", help="mueve las candidatas a archive/")
    ap.add_argument("--transcripts", type=Path, default=Path.home() / ".claude" / "projects")
    a = ap.parse_args(argv)
    try:
        cands = candidatas(vault.VAULT, a.transcripts, hoy or date.today())
    except CoberturaInsuficiente as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    for p in cands:
        print(p)
    if not a.aplicar:
        print(f"{len(cands)} candidatas (sin --aplicar no se mueve nada)")
        return 0
    choques = [p.name for p in cands if (vault.VAULT / "archive" / p.name).exists()]
    if choques:  # todo o nada: ningun movimiento si alguna choca
        print(f"error: archive/ ya tiene {', '.join(choques)}; no se movio nada", file=sys.stderr)
        return 2
    tags: set[str] = set()
    for p in cands:
        tags.update(vault._members(vault._read_frontmatter_only(p)))
        vault._retire(p, None)
        vault.log_op("archive", p.stem)
    for t in sorted(tags):
        if vault._moc_path(t).exists():
            vault.regenerate_moc(t)
    print(f"{len(cands)} archivadas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
