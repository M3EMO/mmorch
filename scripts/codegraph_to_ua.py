"""codegraph -> Understand-Anything: exporta .codegraph/codegraph.db al schema
knowledge-graph.json que consume el dashboard de UA.

Por que: el pipeline LLM de UA cuesta ~181k tokens por repo CHICO reconstruyendo
la estructura que codegraph ya tiene indexada exacta y gratis. Este puente da el
dashboard (grafo interactivo + capas) con costo cero y cero alucinacion de edges.
Trade-off explicito v1: summaries/capas heuristicos (no narrativa LLM) y sin tour
guiado — para narrativa, correr el /understand real o un pase mmorch (futuro).

Uso:
    python scripts/codegraph_to_ua.py <repo>          # <repo>/.codegraph -> <repo>/.ua
    python scripts/codegraph_to_ua.py <repo> --out X  # destino alternativo
"""
from __future__ import annotations

import argparse
import json
import sqlite3
from collections import defaultdict
from pathlib import Path

KIND_MAP = {"file": "file", "class": "class", "function": "function", "method": "function"}
DOC_EXT = {".md", ".rst", ".txt"}
CFG_EXT = {".json", ".toml", ".yaml", ".yml", ".ini", ".cfg"}
EDGE_MAP = {"calls": "calls", "contains": "contains", "imports": "imports",
            "instantiates": "depends_on"}


def _complexity(start: int | None, end: int | None) -> str:
    span = (end or 0) - (start or 0)
    return "simple" if span < 30 else ("moderate" if span < 120 else "complex")


def _file_type(path: str) -> str:
    ext = Path(path).suffix.lower()
    if ext in DOC_EXT:
        return "document"
    if ext in CFG_EXT:
        return "config"
    return "file"


def convert(repo: Path) -> dict:
    db = repo / ".codegraph" / "codegraph.db"
    if not db.exists():
        raise SystemExit(f"sin indice: {db} — correr `codegraph init -i` en {repo}")
    con = sqlite3.connect(db)
    con.row_factory = sqlite3.Row

    nodes, keep = [], {}
    for r in con.execute("SELECT id, kind, name, qualified_name, file_path, start_line, end_line FROM nodes"):
        ua_type = KIND_MAP.get(r["kind"])
        if ua_type is None:  # import/variable = ruido para el dashboard
            continue
        ua_type = _file_type(r["file_path"]) if ua_type == "file" else ua_type
        keep[r["id"]] = r["id"]
        nodes.append({
            "id": r["id"], "type": ua_type, "name": r["name"],
            "filePath": r["file_path"],
            "summary": f"{r['kind']} {r['qualified_name'] or r['name']}"
                       + (f" (lineas {r['start_line']}-{r['end_line']})" if r["start_line"] else ""),
            "tags": [r["kind"]],
            "complexity": _complexity(r["start_line"], r["end_line"]),
        })

    edges = []
    for r in con.execute("SELECT source, target, kind FROM edges"):
        if r["source"] in keep and r["target"] in keep and r["kind"] in EDGE_MAP:
            edges.append({"source": r["source"], "target": r["target"],
                          "type": EDGE_MAP[r["kind"]], "direction": "forward", "weight": 1})

    # imports file->file: resolver via nodos import (name = modulo importado)
    file_by_stem = {Path(n["filePath"]).stem: n["id"] for n in nodes if n["type"] in ("file", "config")}
    seen_imports = set()
    for r in con.execute(
        "SELECT n.name AS mod, n.file_path AS src FROM nodes n WHERE n.kind='import'"
    ):
        src_id = f"file:{r['src']}"
        tgt_id = file_by_stem.get(r["mod"].split(".")[-1])
        if src_id in keep and tgt_id and tgt_id != src_id and (src_id, tgt_id) not in seen_imports:
            seen_imports.add((src_id, tgt_id))
            edges.append({"source": src_id, "target": tgt_id, "type": "imports",
                          "direction": "forward", "weight": 1})

    # capas heuristicas: por directorio top-level de los files
    by_dir: dict[str, list[str]] = defaultdict(list)
    for n in nodes:
        if n["type"] in ("file", "document", "config"):
            parts = Path(n["filePath"]).parts
            by_dir[parts[0] if len(parts) > 1 else "(raiz)"].append(n["id"])
    layers = [{"id": f"layer:{d}", "name": d,
               "description": f"Archivos bajo {d}/" if d != "(raiz)" else "Archivos en la raiz del repo",
               "nodeIds": ids}
              for d, ids in sorted(by_dir.items(), key=lambda kv: -len(kv[1]))]

    langs = sorted({r["language"] for r in con.execute("SELECT DISTINCT language FROM files") if r["language"]})
    con.close()
    import datetime as dt
    import subprocess
    try:
        commit = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"],
                                capture_output=True, text=True, timeout=10).stdout.strip()
    except Exception:
        commit = ""
    # el validador del dashboard exige version string + analyzedAt/gitCommitHash
    return {
        "version": "1.0.0",
        "project": {"name": repo.name, "languages": langs, "frameworks": [],
                    "description": f"Grafo exportado de codegraph ({len(nodes)} nodos, "
                                   f"{len(edges)} edges). Estructura exacta del indice; "
                                   "summaries/capas heuristicos (sin pase LLM).",
                    "analyzedAt": dt.datetime.now(dt.timezone.utc).isoformat(),
                    "gitCommitHash": commit},
        "nodes": nodes, "edges": edges, "layers": layers, "tour": [],
    }


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("repo", type=Path)
    p.add_argument("--out", type=Path, default=None)
    a = p.parse_args()
    graph = convert(a.repo.resolve())
    out_dir = a.out or (a.repo.resolve() / ".ua")
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / "knowledge-graph.json"
    tmp = out.with_suffix(".json.tmp")
    tmp.write_text(json.dumps(graph, ensure_ascii=False), encoding="utf-8")
    tmp.replace(out)
    print(f"{len(graph['nodes'])} nodos, {len(graph['edges'])} edges, "
          f"{len(graph['layers'])} capas -> {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
