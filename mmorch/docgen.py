"""docgen — mmorch documenta SU PROPIO README por introspeccion. El codigo es la
fuente de verdad: módulos (1ra linea del docstring), tools MCP (regex en
mcp_server.py), conteo de tests. Reemplaza el contenido entre marcadores
`<!-- mmorch:auto:NAME -->` ... `<!-- /mmorch:auto:NAME -->` en el README.

Uso:
    python -m mmorch.docgen            # regenera ~/.claude/orchestration/README.md
    from mmorch.docgen import update_readme; update_readme()
"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PKG = ROOT / "mmorch"
README = ROOT / "README.md"

_SKIP = {"__init__.py", "docgen.py"}


def _first_doc_line(py: Path) -> str:
    txt = py.read_text(encoding="utf-8")
    m = re.search(r'^\s*(?:"""|\'\'\')(.+?)(?:\n|"""|\'\'\')', txt, re.S)
    if not m:
        return ""
    line = m.group(1).strip().splitlines()[0].strip()
    return line


def module_table() -> str:
    rows = ["| Módulo | Qué hace |", "|---|---|"]
    for py in sorted(PKG.glob("*.py")):
        if py.name in _SKIP:
            continue
        desc = _first_doc_line(py) or "(sin docstring)"
        rows.append(f"| `mmorch/{py.name}` | {desc} |")
    return "\n".join(rows)


def mcp_tools() -> list[str]:
    src = (ROOT / "mcp_server.py")
    if not src.exists():
        return []
    txt = src.read_text(encoding="utf-8")
    return sorted(set(re.findall(r"def (mmorch_\w+)\s*\(", txt)))


def stats() -> dict:
    n_mod = len([p for p in PKG.glob("*.py") if p.name not in _SKIP])
    tdir = ROOT / "tests"
    n_tests = 0
    if tdir.exists():
        for tf in tdir.glob("test_*.py"):
            n_tests += len(re.findall(r"^\s*def test_\w+", tf.read_text(encoding="utf-8"), re.M))
    return {"modules": n_mod, "tools": len(mcp_tools()), "tests": n_tests}


def render_block(name: str) -> str:
    if name == "modules":
        return module_table()
    if name == "tools":
        ts = mcp_tools()
        return "MCP tools (server `mmorch`): " + ", ".join(f"`{t}`" for t in ts) + \
            ".\n\n**Restart Claude Code** to load new tools."
    if name == "stats":
        s = stats()
        return (f"_Auto-generado por `mmorch.docgen`._ "
                f"**{s['modules']} módulos · {s['tools']} MCP tools · {s['tests']} tests.**")
    raise ValueError(f"bloque desconocido: {name}")


def update_readme(path: Path = README) -> list[str]:
    """Reemplaza cada bloque entre marcadores por su contenido fresco. Devuelve
    la lista de bloques actualizados. Idempotente."""
    txt = path.read_text(encoding="utf-8")
    updated = []
    for name in ("modules", "tools", "stats"):
        start = f"<!-- mmorch:auto:{name} -->"
        end = f"<!-- /mmorch:auto:{name} -->"
        pat = re.compile(re.escape(start) + r".*?" + re.escape(end), re.S)
        if not pat.search(txt):
            continue
        block = f"{start}\n{render_block(name)}\n{end}"
        txt = pat.sub(lambda _m: block, txt)
        updated.append(name)
    path.write_text(txt, encoding="utf-8")
    return updated


def main() -> None:
    up = update_readme()
    print(f"README actualizado: bloques {up} | {stats()}")


if __name__ == "__main__":
    main()
