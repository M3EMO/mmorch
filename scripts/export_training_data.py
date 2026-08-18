"""Exportador del flywheel: los datos que el loop ya genera -> datasets entrenables.

Corre cuando quieras (o mensual): junta las fuentes vivas y emite JSONL
normalizado en training/ (gitignoreado — datos, no fuente). Cada linea:
{"kind", "input", "output"|"label", "meta"}. Redaccion: los traces son
local-only; el export pasa por el redact de sessions si esta disponible.

Fuentes -> datasets:
  logs/idea_loop_traces.jsonl  -> sft_judge.jsonl      (prompt -> output json)
  logs/feedback.jsonl          -> router_prefs.jsonl   (arm/pattern/context -> reward)
  logs/adjudications.json      -> match_labels.jsonl   (nota+proyecto -> strong/status)
  vault/roadmaps/candidatos.md -> idea_labels.jsonl    (gist -> promovida/expirada/pendiente)

Uso: .venv/Scripts/python.exe scripts/export_training_data.py
"""

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

OUT = ROOT / "training"


def _write(name: str, rows: list) -> int:
    OUT.mkdir(exist_ok=True)
    with open(OUT / name, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False, default=str) + "\n")
    return len(rows)


def _jsonl(path: pathlib.Path) -> list:
    try:
        out = []
        for line in path.read_text(encoding="utf-8").splitlines():
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
        return out
    except OSError:
        return []


def export_judge_sft() -> int:
    rows = [{"kind": "sft_judge", "input": t.get("prompt", ""),
             "output": t.get("output", {}),
             "meta": {"model": t.get("model"), "ts": t.get("ts"),
                      "temperature": t.get("temperature")}}
            for t in _jsonl(ROOT / "logs" / "idea_loop_traces.jsonl")
            if t.get("prompt") and t.get("output")]
    return _write("sft_judge.jsonl", rows)


def export_router_prefs() -> int:
    rows = [{"kind": "router_pref",
             "input": {"arm": e.get("arm"), "pattern": e.get("pattern"),
                       "context": e.get("context", "")},
             "label": e.get("reward"),
             "meta": {"source": e.get("source"), "ts": e.get("ts")}}
            for e in _jsonl(ROOT / "logs" / "feedback.jsonl")
            if e.get("arm") is not None and e.get("reward") is not None]
    return _write("router_prefs.jsonl", rows)


def export_match_labels() -> int:
    try:
        adj = json.loads((ROOT / "logs" / "adjudications.json")
                         .read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return _write("match_labels.jsonl", [])
    rows = []
    for entry in (adj.get("pairs") or {}).values():
        r = entry.get("result", {})
        if r.get("skipped"):
            continue
        rows.append({"kind": "match_label",
                     "input": {"note_path": r.get("note_path"),
                               "project": r.get("project")},
                     "label": {"score": r.get("score"), "strong": r.get("strong"),
                               "status": r.get("status"),
                               "justification": r.get("justification")},
                     "meta": {"hash": entry.get("hash")}})
    return _write("match_labels.jsonl", rows)


def export_idea_labels() -> int:
    from mmorch.fuel import parse_archivadas, parse_candidatos
    try:
        md = (ROOT / "vault" / "roadmaps" / "candidatos.md").read_text(encoding="utf-8")
    except OSError:
        return _write("idea_labels.jsonl", [])
    rows = [{"kind": "idea_label", "input": e["gist"],
             "label": e["estado"],
             "meta": {"id": e["id"], "lente": e["lente"], "fecha": e["fecha"]}}
            for e in parse_candidatos(md) + parse_archivadas(md)]
    return _write("idea_labels.jsonl", rows)


def export_dpo_pairs() -> int:
    rows = [{"kind": "dpo_pair",
             "input": {"rubric": t.get("rubric"), "artifact": t.get("artifact")},
             "label": {"passed": t.get("passed"), "confidence": t.get("confidence"),
                       "refutations": t.get("refutations", [])},
             "meta": {"gen": t.get("gen_model"), "verifier": t.get("verifier_model"),
                      "phase": t.get("phase"), "ts": t.get("ts")}}
            for t in _jsonl(ROOT / "logs" / "dpo_pairs.jsonl")]
    return _write("dpo_pairs.jsonl", rows)


def main() -> None:
    counts = {"sft_judge": export_judge_sft(),
              "dpo_pairs": export_dpo_pairs(),
              "router_prefs": export_router_prefs(),
              "match_labels": export_match_labels(),
              "idea_labels": export_idea_labels()}
    print(json.dumps({"exported": counts, "dir": str(OUT)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
