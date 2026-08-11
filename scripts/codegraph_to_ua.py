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
    for r in con.execute("SELECT id, kind, name, qualified_name, file_path, start_line, end_line,"
                         " docstring, signature FROM nodes"):
        ua_type = KIND_MAP.get(r["kind"])
        if ua_type is None:  # import/variable = ruido para el dashboard
            continue
        ua_type = _file_type(r["file_path"]) if ua_type == "file" else ua_type
        keep[r["id"]] = r["id"]
        # docstring del indice > heuristica: primera linea, es la mejor sintesis gratis
        doc = (r["docstring"] or "").strip().splitlines()
        summary = doc[0].strip() if doc else (
            f"{r['kind']} {r['qualified_name'] or r['name']}"
            + (f"{r['signature']}" if r["signature"] else "")
            + (f" (lineas {r['start_line']}-{r['end_line']})" if r["start_line"] else ""))
        nodes.append({
            "id": r["id"], "type": ua_type, "name": r["name"],
            "filePath": r["file_path"],
            "summary": summary,
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


_TOUR_SCHEMA = {
    "type": "object",
    "properties": {
        "project_description": {"type": "string"},
        "layers": {"type": "object"},
        "tour": {"type": "array", "items": {
            "type": "object",
            "properties": {"order": {"type": "number"}, "title": {"type": "string"},
                           "description": {"type": "string"},
                           "nodeIds": {"type": "array", "items": {"type": "string"}}},
            "required": ["order", "title", "description", "nodeIds"]}},
    },
    "required": ["project_description", "layers", "tour"],
}


def narrate(graph: dict) -> dict:
    """Pase LLM barato (DeepSeek via mmorch, centavos): descripcion de proyecto/capas
    + tour guiado, generado SOLO sobre el grafo exacto (docstrings incluidos).
    nodeIds inventados se descartan (validacion contra el grafo, no confianza).
    Falla -> el grafo queda sin narrar (la narrativa es side-channel, no gate)."""
    from mmorch.schema import SchemaGateError, gated_json

    valid_ids = {n["id"] for n in graph["nodes"]}
    hub_fanin: dict[str, int] = {}
    for e in graph["edges"]:
        hub_fanin[e["target"]] = hub_fanin.get(e["target"], 0) + 1
    hubs = sorted((n for n in graph["nodes"] if n["type"] != "function"),
                  key=lambda n: -hub_fanin.get(n["id"], 0))[:40]
    ctx = {
        "project": graph["project"]["name"],
        "layers": [{"id": ly["id"], "name": ly["name"], "n_files": len(ly["nodeIds"]),
                    "files": ly["nodeIds"][:25]} for ly in graph["layers"]],
        "hubs": [{"id": n["id"], "fan_in": hub_fanin.get(n["id"], 0),
                  "summary": n["summary"][:150]} for n in hubs],
    }
    prompt = (
        "Sos un guia de codebases. Con este grafo (capas, archivos, hubs por fan-in y "
        "sus docstrings) genera:\n"
        "1. project_description: 2 frases de que es el proyecto.\n"
        "2. layers: dict id_de_capa -> descripcion de 1-2 frases del ROL de esa capa.\n"
        "3. tour: 6-10 pasos ordenados por dependencia (entry points primero) para "
        "aprender el repo; cada paso con title, description (3-4 frases, concreto, "
        "menciona archivos por nombre) y nodeIds (SOLO ids que aparecen en el contexto).\n"
        "Responde JSON.\n\n" + json.dumps(ctx, ensure_ascii=False)
    )
    try:
        out = gated_json("deepseek-chat", [{"role": "user", "content": prompt}],
                         schema=_TOUR_SCHEMA, phase="ua_narrate")
    except SchemaGateError as e:
        print(f"[narrate] fallo el schema gate, grafo queda sin tour: {e}")
        return graph
    graph["project"]["description"] = out["project_description"]
    for ly in graph["layers"]:
        if ly["id"] in out["layers"]:
            ly["description"] = str(out["layers"][ly["id"]])
    # el dashboard arranca en modo "Files": si un paso solo referencia clases/
    # funciones, el zoom no tiene target visible — incluir siempre el file padre
    file_of = {n["id"]: f"file:{n['filePath']}" for n in graph["nodes"]
               if n["type"] not in ("file", "document", "config")}
    tour = []
    for i, step in enumerate(out["tour"], 1):
        ids = [x for x in step["nodeIds"] if x in valid_ids]  # anti-alucinacion
        parents = [file_of[x] for x in ids if x in file_of and file_of[x] in valid_ids]
        ids = list(dict.fromkeys(parents + ids))  # files primero, sin dups
        if ids:
            tour.append({"order": i, "title": step["title"],
                         "description": step["description"], "nodeIds": ids})
    graph["tour"] = tour
    print(f"[narrate] tour de {len(tour)} pasos + {len(out['layers'])} capas narradas")
    return graph


_FILES_SCHEMA = {
    "type": "object",
    "properties": {"summaries": {"type": "object"}},
    "required": ["summaries"],
}


def narrate_files(graph: dict, batch_size: int = 25) -> dict:
    """Summaries DeepSeek POR ARCHIVO (batches, centavos). Contexto por archivo:
    docstring del modulo + simbolos contenidos con sus firmas. Un batch que falla
    se saltea (los demas quedan); ids desconocidos se descartan."""
    from mmorch.schema import SchemaGateError, gated_json

    by_id = {n["id"]: n for n in graph["nodes"]}
    contained: dict[str, list[str]] = defaultdict(list)
    for e in graph["edges"]:
        if e["type"] == "contains" and e["source"] in by_id and e["target"] in by_id:
            t = by_id[e["target"]]
            contained[e["source"]].append(f"{t['name']}: {t['summary'][:90]}")
    targets = [n for n in graph["nodes"] if n["type"] in ("file", "document", "config")]
    done = 0
    for i in range(0, len(targets), batch_size):
        batch = targets[i:i + batch_size]
        ctx = [{"id": n["id"], "path": n["filePath"], "doc": n["summary"][:150],
                "symbols": contained.get(n["id"], [])[:12]} for n in batch]
        prompt = (
            "Para cada archivo, escribi un summary de 1-2 frases: QUE hace y su rol "
            "en el sistema (no listes los simbolos — sintetiza). Responde JSON "
            '{"summaries": {"<id>": "<summary>", ...}} cubriendo TODOS los ids.\n\n'
            + json.dumps(ctx, ensure_ascii=False)
        )
        try:
            out = gated_json("deepseek-chat", [{"role": "user", "content": prompt}],
                             schema=_FILES_SCHEMA, phase="ua_narrate_files")
        except SchemaGateError as e:
            print(f"[narrate-files] batch {i // batch_size + 1} fallo, sigo: {e}")
            continue
        for nid, s in out["summaries"].items():
            if nid in by_id and isinstance(s, str) and s.strip():
                by_id[nid]["summary"] = s.strip()
                done += 1
    print(f"[narrate-files] {done}/{len(targets)} archivos narrados")
    return graph


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("repo", type=Path)
    p.add_argument("--out", type=Path, default=None)
    p.add_argument("--narrate", action="store_true",
                   help="pase DeepSeek (centavos) para descripcion de capas + tour")
    p.add_argument("--narrate-files", action="store_true",
                   help="summaries DeepSeek por archivo (batches, centavos)")
    a = p.parse_args()
    graph = convert(a.repo.resolve())
    if a.narrate_files:
        graph = narrate_files(graph)  # antes del tour: el tour lee summaries mejores
    if a.narrate:
        graph = narrate(graph)
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
