"""minds — global federation graph across registered projects (read-only).

Shape matches the Lotus client: {nodes:[{id,project,label,kind,indexed}], edges:[{from,to,kind}]}.

ponytail: shallow scandir (root + one level), NOT a full rglob — one registered
"project" is the home dir, so an unbounded walk would be catastrophic. We don't
parse codegraph's SQLite (schema is its own concern); we just flag which projects
are indexed. Cross-project edges = path containment (real + cheap), e.g. Lotus ⊂ Claude.
"""
from __future__ import annotations

import os
from pathlib import Path

_EXT = {".py", ".js", ".ts", ".tsx", ".jsx", ".rs", ".go", ".java", ".rb", ".css", ".html", ".md"}
_SKIP = {".git", "node_modules", ".venv", "venv", "__pycache__", ".codegraph",
         "dist", "build", ".next", "target", ".idea", ".vscode", "AppData"}
_SCAN_CAP = 500  # max dir entries scanned per directory — bounds cost on huge dirs


def _top_files(root: str, limit: int) -> list[str]:
    """A few source files near the root: root level first, then one directory deep."""
    out: list[str] = []
    try:
        entries = list(os.scandir(root))
    except OSError:
        return out
    for e in entries[:_SCAN_CAP]:
        if len(out) >= limit:
            return out
        try:
            if e.is_file() and Path(e.name).suffix in _EXT:
                out.append(e.name)
        except OSError:
            continue
    for e in entries:
        if len(out) >= limit:
            return out
        if not e.name.startswith(".") and e.name not in _SKIP:
            try:
                if not e.is_dir():
                    continue
                for f in list(os.scandir(e.path))[:_SCAN_CAP]:
                    if len(out) >= limit:
                        return out
                    if f.is_file() and Path(f.name).suffix in _EXT:
                        out.append(f"{e.name}/{f.name}")
            except OSError:
                continue
    return out


def federation(max_files_per_project: int = 6) -> dict:
    from .projects import list_projects
    items = [(n, p) for n, p in list_projects().items() if os.path.isdir(p)]
    nodes: list[dict] = []
    edges: list[dict] = []

    for name, path in items:
        indexed = os.path.isdir(os.path.join(path, ".codegraph"))
        nodes.append({"id": name, "project": name, "label": name,
                      "kind": "project", "indexed": indexed})
        for rel in _top_files(path, max_files_per_project):
            nid = f"{name}:{rel}"
            nodes.append({"id": nid, "project": name, "label": rel, "kind": "module"})
            edges.append({"from": nid, "to": name, "kind": "concept"})

    # cross-project containment: project B nested under project A's path
    for an, ap in items:
        ap_pref = os.path.normpath(ap) + os.sep
        for bn, bp in items:
            if an != bn and os.path.normpath(bp).startswith(ap_pref):
                edges.append({"from": bn, "to": an, "kind": "dep"})

    return {"nodes": nodes, "edges": edges}


if __name__ == "__main__":
    g = federation()
    assert g["nodes"], "federation produced no nodes"
    assert all({"id", "project", "label", "kind"} <= set(n) for n in g["nodes"]), "node shape"
    assert all({"from", "to", "kind"} <= set(e) for e in g["edges"]), "edge shape"
    projects = [n for n in g["nodes"] if n["kind"] == "project"]
    print(f"minds OK: {len(projects)} projects, {len(g['nodes'])} nodes, {len(g['edges'])} edges")
