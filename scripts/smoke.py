"""Smoke-run de subsistemas de mmorch — un chequeo VIVO por módulo, barato y
read-only (cero LLM). Verifica uso correcto de las piezas, no lógica interna
(eso es de la suite). Uso: .venv/Scripts/python.exe scripts/smoke.py"""

import io
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

G, R, D, B, X = "\x1b[92m", "\x1b[91m", "\x1b[2m", "\x1b[1m", "\x1b[0m"


def check(name):
    def deco(fn):
        def run():
            try:
                detail = fn()
                print(f"  {G}✓{X} {name:<22} {D}{detail}{X}")
                return True
            except Exception as e:
                print(f"  {R}✗{X} {B}{name:<22}{X} {R}{type(e).__name__}: {str(e)[:90]}{X}")
                return False
        return run
    return deco


@check("fuel (candidatas)")
def c_fuel():
    from mmorch.fuel import parse_archivadas, parse_candidatos
    md = (ROOT / "vault" / "roadmaps" / "candidatos.md").read_text(encoding="utf-8")
    v, a = parse_candidatos(md), parse_archivadas(md)
    return f"{len(v)} vigentes, {len(a)} archivadas"


@check("curation (pending)")
def c_curation():
    from mmorch.curation import pending
    p = pending()
    return f"{len(p['candidatas'])} candidatas, {len(p['cards'])} tarjetas"


@check("adjudicate (estado)")
def c_adj():
    adj = json.loads((ROOT / "logs" / "adjudications.json").read_text(encoding="utf-8"))
    n = sum(len(v) for v in adj.get("by_project", {}).values())
    return f"{len(adj.get('pairs', {}))} pares juzgados, {n} strong"


@check("health (report)")
def c_health():
    from mmorch.health import report
    r = report(logs_dir=str(ROOT / "logs"))
    return f"healthy={r['healthy']}, dead={len(r['check']['dead'])}, never={len(r['check']['never'])}"


@check("outcomes (feedback)")
def c_outcomes():
    from mmorch.feedback import ThompsonBandit
    stats = ThompsonBandit().stats()
    verd = sum(1 for line in (ROOT / "logs" / "feedback.jsonl")
               .read_text(encoding="utf-8").splitlines() if '"verdict"' in line)
    return f"{len(stats)} brazos, {verd} outcomes con veredicto humano"


@check("automerge (semaforo)")
def c_automerge():
    from mmorch.automerge import classify_branch
    base = subprocess.run(["git", "rev-parse", "--abbrev-ref", "HEAD"],
                          cwd=ROOT, capture_output=True, text=True).stdout.strip()
    out = subprocess.run(["git", "branch", "--list", "--no-merged", base,
                          "mmorch/*", "--format=%(refname:short)"],
                         cwd=ROOT, capture_output=True, text=True)
    branches = [b for b in out.stdout.splitlines() if b.strip()][:3]
    zones = {b: classify_branch(str(ROOT), b, base=base).get("zone") for b in branches}
    return f"base={base}, {zones or 'sin branches pendientes'}"


@check("merge_train (cola)")
def c_train():
    from mmorch.merge_train import yellow_branches
    base = subprocess.run(["git", "rev-parse", "--abbrev-ref", "HEAD"],
                          cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return f"{len(yellow_branches(str(ROOT), base=base))} amarillas en cola"


@check("decision_mining")
def c_decisions():
    p = ROOT / "logs" / "decision_samples.jsonl"
    n = len(p.read_text(encoding="utf-8").splitlines()) if p.exists() else 0
    return f"{n} decisiones gold acumuladas"


@check("flywheel (training/)")
def c_training():
    t = ROOT / "training"
    counts = {f.stem: len(f.read_text(encoding="utf-8").splitlines())
              for f in sorted(t.glob("*.jsonl"))} if t.exists() else {}
    return f"{sum(counts.values())} ejemplos en {len(counts)} datasets"


@check("descubrimiento (exogeno)")
def c_discovery():
    """Las dos fuentes que pueden nombrar un tema que el sistema no conoce."""
    from mmorch.bursts import bursting
    from mmorch.frontier import frontier
    logs = str(ROOT / "logs")
    from mmorch.iohelpers import load_json_tolerant
    g = load_json_tolerant(ROOT / "logs" / "topic_graph.json", {})
    ar = load_json_tolerant(ROOT / "logs" / "arxiv_terms.json", {})
    return (f"grafo {len(g.get('nodes', {}))} topics/{g.get('docs', 0)} repos "
            f"-> frontera {frontier(logs_dir=logs, k=3)}; arxiv "
            f"{len(ar.get('weeks', {}))} semanas -> bursts "
            f"{bursting(logs_dir=logs, k=3)}")


@check("reflexion (historia)")
def c_reflect():
    p = ROOT / "logs" / "reflexiones.jsonl"
    n = len(p.read_text(encoding="utf-8").splitlines()) if p.exists() else 0
    return f"{n} reflexiones registradas"


@check("budget (loop)")
def c_budget():
    from mmorch.iohelpers import load_json_tolerant
    b = load_json_tolerant(ROOT / "logs" / "loop_budget.json", {})
    return f"mes {b.get('month')}: {b.get('calls', 0)} calls, USD {b.get('usd', 0):.4f}"


@check("server (endpoints)")
def c_server():
    import re
    import urllib.request
    tok = ""
    for line in (ROOT / ".env").read_text(encoding="utf-8").splitlines():
        m = re.match(r"MMORCH_SERVER_TOKEN=(.+)", line.strip())
        if m:
            tok = m.group(1)
            break
    d = json.loads(urllib.request.urlopen(
        f"http://127.0.0.1:8787/pending?token={tok}", timeout=5).read().decode())
    return f"/pending vivo ({len(d['candidatas'])}+{len(d['cards'])})"


def main() -> None:
    import os
    os.system("")
    import time
    checks = [("fuel", c_fuel), ("curation", c_curation), ("adjudicate", c_adj),
              ("health", c_health), ("outcomes", c_outcomes),
              ("automerge", c_automerge), ("merge_train", c_train),
              ("decision_mining", c_decisions), ("flywheel", c_training),
              ("descubrimiento", c_discovery),
              ("reflexion", c_reflect), ("budget", c_budget),
              ("server", c_server)]
    print(f"\n{B}SMOKE mmorch — {len(checks)} subsistemas{X}\n")
    fails = [name for name, c in checks if not c()]
    ok = len(checks) - len(fails)
    color = G if not fails else R
    print(f"\n{color}{B}{ok}/{len(checks)} OK{X}\n")
    # log system: historia append-only para tendencia + consumo de nightly/digest
    try:
        with open(ROOT / "logs" / "smoke.jsonl", "a", encoding="utf-8") as f:
            f.write(json.dumps({"ts": time.time(), "ok": ok,
                                "total": len(checks), "fails": fails},
                               ensure_ascii=False) + "\n")
    except OSError:
        pass
    sys.exit(0 if not fails else 1)


if __name__ == "__main__":
    main()
