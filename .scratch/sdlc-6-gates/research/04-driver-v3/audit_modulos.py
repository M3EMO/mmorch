"""Auditoria de TODOS los modulos de mmorch (2026-09-14), regla del usuario:
funciona? -> util? -> cableable cerrado / activacion automatica / uso recurrente? -> si no, borrar.

Evidencia por modulo, toda computada:
- entrada: por que tipo de entrada se alcanza (auto = nightly/auto_apply_nightly/loop_nightly;
  recurrente = server/mcp_server; manual = cli/scripts/hillclimb/autoresearch; ninguna).
- tests: cuantos archivos de test lo importan; fallos de esos tests segun logs/audit-junit.xml.
- uso90d: hits de pattern/tool en metrics.jsonl + mcp_calls.jsonl (solo modulos que llaman modelos/tools).
- selfcheck: tiene bloque __main__.
- proposito: primera linea del docstring.
Salida: markdown a stdout.
"""
import ast
import collections
import glob
import json
import os
import re
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, "tests")
import test_capas as T  # noqa: E402

REPO = Path(".")
mods = T._modulos()
allm = set(mods)
imports = {m: T._imports(p, allm) for m, p in mods.items()}
importers = collections.defaultdict(set)
for m, deps in imports.items():
    for d in deps:
        importers[d].add(m)

ENTRY_KIND = {"nightly": "auto", "auto_apply_nightly": "auto", "loop_nightly": "auto",
              "server": "recurrente", "mcp_server": "recurrente",
              "cli": "manual", "hillclimb": "manual", "autoresearch": "manual"}
for f in glob.glob("scripts/*.py"):
    src = Path(f).read_text(encoding="utf-8", errors="replace")
    for a, b in re.findall(r"from mmorch(?:\.(\w+))? import|import mmorch\.(\w+)", src):
        for x in (a, b):
            if x in mods:
                ENTRY_KIND.setdefault(x, "manual")
ENTRY_KIND["loop"] = ENTRY_KIND.get("loop", "auto")

def reach_kinds(m):
    kinds = set()
    for e, k in ENTRY_KIND.items():
        if e not in mods:
            continue
        seen, stack = {e}, [e]
        while stack:
            x = stack.pop()
            if x == m:
                kinds.add(k); break
            for d in imports.get(x, ()):
                if d not in seen:
                    seen.add(d); stack.append(d)
    return kinds

# tests por modulo
tests_of = collections.defaultdict(set)
for f in glob.glob("tests/*.py"):
    src = Path(f).read_text(encoding="utf-8", errors="replace")
    for m in mods:
        if re.search(r"mmorch\." + m + r"\b|from mmorch import [^\n]*\b" + m + r"\b", src):
            tests_of[m].add(Path(f).stem)
fails_by_file = collections.Counter()
tests_by_file = collections.Counter()
if os.path.exists("logs/audit-junit.xml"):
    for tc in ET.parse("logs/audit-junit.xml").getroot().iter("testcase"):
        fn = (tc.get("classname") or "").split(".")[-1]
        tests_by_file[fn] += 1
        if tc.find("failure") is not None or tc.find("error") is not None:
            fails_by_file[fn] += 1

cut = time.time() - 90 * 86400
pat, tools = collections.Counter(), collections.Counter()
for l in open("logs/metrics.jsonl", encoding="utf-8", errors="replace"):
    try:
        r = json.loads(l)
    except json.JSONDecodeError:
        continue
    if float(r.get("ts") or 0) >= cut:
        pat[str(r.get("pattern") or "")] += 1
if os.path.exists("logs/mcp_calls.jsonl"):
    for l in open("logs/mcp_calls.jsonl", encoding="utf-8", errors="replace"):
        try:
            r = json.loads(l)
        except json.JSONDecodeError:
            continue
        if float(r.get("ts") or 0) >= cut:
            tools[str(r.get("tool") or r.get("name") or "")] += 1

def hits(m):
    return sum(c for p, c in pat.items() if m in p) + sum(c for t, c in tools.items() if m in t)

def purpose(p):
    try:
        d = ast.get_docstring(ast.parse(p.read_text(encoding="utf-8", errors="replace"))) or ""
    except SyntaxError:
        d = ""
    return d.strip().splitlines()[0][:90] if d.strip() else "(sin docstring)"

rows = []
for m, p in sorted(mods.items()):
    src = p.read_text(encoding="utf-8", errors="replace")
    kinds = reach_kinds(m)
    tf = sorted(tests_of[m])
    nt = sum(tests_by_file.get(t, 0) for t in tf)
    nf = sum(fails_by_file.get(t, 0) for t in tf)
    funciona = "sin medir" if not tf else ("si" if nf == 0 else f"NO ({nf}/{nt} rojos)")
    cable = "+".join(sorted(kinds)) if kinds else "NINGUNA"
    h = hits(m)
    if not kinds:
        verdict = "borrar o cablear"
    elif funciona.startswith("NO"):
        verdict = "arreglar o borrar"
    elif "auto" in kinds or "recurrente" in kinds:
        verdict = "queda"
    elif h == 0:
        verdict = "manual sin uso: medir o borrar"
    else:
        verdict = "manual con uso: queda"
    rows.append((verdict, m, src.count("\n"), cable, ",".join(tf) or "-", funciona, h,
                 "si" if '__name__ == "__main__"' in src else "-", len(importers[m]), purpose(p)))

order = ["borrar o cablear", "arreglar o borrar", "manual sin uso: medir o borrar", "manual con uso: queda", "queda"]
rows.sort(key=lambda r: (order.index(r[0]), -r[2]))
print("| veredicto | modulo | loc | entrada | tests | funciona | uso90d | selfcheck | importadores | proposito |")
print("|---|---|---|---|---|---|---|---|---|---|")
for r in rows:
    print("| " + " | ".join(str(x) for x in r) + " |")
print()
print("resumen:", collections.Counter(r[0] for r in rows).most_common())
