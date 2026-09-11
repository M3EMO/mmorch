"""PROTOTIPO throwaway — driver v3 para tasks Python del bench (ticket 06/07). No es el engine vivo.

Mismo pipeline que driver_v3.py (Java) sobre pytest: spec -> plan -> build -> test -> review Claude -> pr.
Gates: G1 contrato en spec, plan-allowlist, baseline (tests_accept intacto), py_compile, collect-only,
regresion por unidad, topes por avance (stall / novedad / USD), revision de Claude con test que falla.
Escalera: fix loop 3 -> reasoner x2 -> Claude (edit) -> humano.

Uso:
  python driver_py.py --task rate-limiter --wt <dir> [--phase sdlc-rate-limiter-r1]
  python driver_py.py --self-check
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import time

sys.path.insert(0, r"C:\Users\map12\.claude\orchestration")
from mmorch.providers import call  # noqa: E402
from mmorch import bench  # noqa: E402


def _arg(flag, default):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default


TASK_NAME = _arg("--task", "rate-limiter")
WT = pathlib.Path(_arg("--wt", rf"C:\Users\map12\Desktop\Claude\sdlc-runs\{TASK_NAME}-r1"))
PHASE = _arg("--phase", f"sdlc-{TASK_NAME}-r1")
MAX_FIX = int(_arg("--max-fix", "3"))
REASONER_TRIES = 2
WRITER, CODER = "deepseek-reasoner", "deepseek-v4-pro"
PY = sys.executable
LOG = WT / "docs" / "sdlc" / "run-log.json"
METRICS = pathlib.Path(r"C:\Users\map12\.claude\orchestration\logs\metrics.jsonl")
REVIEW_REL = "tests_accept/test_review.py"
PY_RE = r"`?((?:\w+/)*\w+\.py)`?"

# Tokens que la spec tiene que nombrar verbatim (G1). Uno por task; se agrega al llegar cada feature.
CONTRACTS = {
    "rate-limiter": ["TokenBucket", "MultiLimiter", "allow", "capacity", "refill_per_s", "now", "key",
                     "limiter/core.py", "limiter/multi.py", "limiter/__init__.py"],
}


def _cfg():
    """Topes del ticket 07. docs/sdlc/sdlc.toml pisa los defaults."""
    d = {"usd_max": 3.0, "stall_rounds": 2, "diff_novelty_min": 0.10, "cmd_timeout_s": 600}
    p = WT / "docs" / "sdlc" / "sdlc.toml"
    if p.exists():
        import tomllib
        d.update(tomllib.loads(p.read_text(encoding="utf-8")))
    return d


CFG = _cfg()

state: dict = {
    "proto": "driver_py", "task": TASK_NAME,
    "stages": [], "gates": [], "calls": 0, "human_interventions": 0,
    "claude_calls": 0, "gate_rejects": [], "escalated_to_claude": False,
}
SNAP: dict[str, str] = {}
SEEN: set[str] = set()


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
             pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
    usd = ledger_usd() or 0
    if usd > CFG["usd_max"]:
        rec_gate("usd-tope", False, f"US${usd} > {CFG['usd_max']}")
        write_supervision(f"ESCALATE_HUMAN: tope USD {usd} superado")
        _flush()
        sys.exit(f"tope USD {usd}: escala a humano")
    return r.text


def rec_gate(gid, ok, detail):
    state["gates"].append({"id": gid, "ok": ok, "detail": detail, "calls": state["calls"]})
    if not ok:
        state["gate_rejects"].append({"gate": gid, "detail": detail})
    return ok, detail


def stage(name):
    def deco(fn):
        def run():
            t0 = time.time(); c0 = state["calls"]
            ok, note = fn()
            state["stages"].append({"stage": name, "ok": ok, "minutes": round((time.time() - t0) / 60, 2),
                                    "calls": state["calls"] - c0, "note": note})
            _flush()
            print(f"[{name}] ok={ok} {note} ({state['calls'] - c0} llamadas, {(time.time() - t0) / 60:.1f} min)", flush=True)
            if not ok:
                sys.exit(f"etapa {name} fallo: {note}")
        return run
    return deco


def _flush():
    LOG.parent.mkdir(parents=True, exist_ok=True)
    LOG.write_text(json.dumps(state, indent=1, ensure_ascii=False), encoding="utf-8")


def strip_fence(t):
    m = re.search(r"```(?:\w+)?\s*(.*?)```", t, re.S)
    return (m.group(1) if m else t).strip()


def sh(cmd: list[str]):
    p = subprocess.run(cmd, cwd=WT, capture_output=True, text=True, encoding="utf-8", errors="replace",
                       timeout=CFG["cmd_timeout_s"])
    return p.returncode == 0, (p.stdout + p.stderr)[-6000:]


def accept():
    return sh([PY, "-m", "pytest", "tests_accept", "-q", "-p", "no:cacheprovider"])


def test_counts(log):
    f = sum(int(x) for x in re.findall(r"(\d+) (?:failed|error)", log))
    p = sum(int(x) for x in re.findall(r"(\d+) passed", log))
    if not (f or p):
        return None if "no tests ran" not in log else {"run": 0, "failed": 10 ** 6}
    return {"run": f + p, "failed": f}


def _novelty(old: dict, new: dict) -> float:
    """Fraccion de lineas agregadas en esta vuelta que nunca aparecieron en vueltas previas (ticket 07)."""
    before = {l.strip() for c in old.values() for l in c.splitlines()}
    added = {l.strip() for c in new.values() for l in c.splitlines()} - before
    SEEN.update(before)
    nov = len(added - SEEN) / max(1, len(added))
    SEEN.update(added)
    return round(nov, 2)


def sha_file(rel):
    return hashlib.sha256((WT / rel).read_bytes()).hexdigest()


def snapshot_baseline():
    for rel in TASK.accept_files:
        SNAP[rel] = sha_file(rel)


def gate_plan_allowlist(plan_md: str, files: list[str]) -> tuple[bool, str]:
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    block = m.group(1) if m else plan_md
    if re.search(r"(?im)^\s*[-*]+\s*`?tests_accept/", block):
        return rec_gate("plan-allowlist", False, "plan lista tests_accept como archivo a escribir")
    need = [f for f in re.findall(r"[\w/]+\.py", TASK.task) if f not in files]
    if need:
        return rec_gate("plan-allowlist", False, f"plan omite {need}")
    return rec_gate("plan-allowlist", True, f"{files}")


def gate_baseline() -> tuple[bool, str]:
    bad = [rel for rel, h in SNAP.items() if not (WT / rel).exists() or sha_file(rel) != h]
    return rec_gate("baseline-intacto", not bad, "ok" if not bad else f"toco {bad}")


def gate_compile(files) -> tuple[bool, str]:
    ok, log = sh([PY, "-m", "py_compile", *files])
    return rec_gate("G3-compile", ok, "compila" if ok else log[-800:])


def gate_test_compile() -> tuple[bool, str]:
    """Equivalente a mvn test-compile: los tests importan y se recolectan."""
    ok, log = sh([PY, "-m", "pytest", "tests_accept", "--collect-only", "-q", "-p", "no:cacheprovider"])
    return rec_gate("test-compile", ok, "ok" if ok else log[-800:])


def ledger_usd():
    if not METRICS.exists():
        return None
    t0 = state.get("t0_epoch") or 0
    usd = 0.0
    state["usd_by_family"] = {}
    for line in METRICS.read_text(encoding="utf-8").splitlines()[-8000:]:
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if r.get("phase") == PHASE and float(r.get("ts") or 0) >= t0:
            c = float(r.get("cost_usd") or 0)
            usd += c
            fam = r.get("family") or "?"
            state["usd_by_family"][fam] = round(state["usd_by_family"].get(fam, 0) + c, 4)
    return round(usd, 4)


def _joined(written):
    return "\n\n".join(f"# {k}\n{v}" for k, v in written.items())


def _all_code():
    return {f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()}


def _write(rel, code):
    p = WT / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(code, encoding="utf-8")


def _tests_text():
    t = "\n\n".join(f"# {k}\n{(WT / k).read_text(encoding='utf-8')}" for k in TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        t += "\n\n# tests_accept/test_review.py (revision de Claude, no se toca)\n" + (WT / REVIEW_REL).read_text(encoding="utf-8")
    return t


def write_supervision(note: str):
    p = WT / "docs" / "sdlc" / "supervision.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    prev = p.read_text(encoding="utf-8") if p.exists() else "# supervision.md — driver_py\n\n"
    p.write_text(prev + f"\n## candidato {time.strftime('%H:%M:%S')}\n{note}\n", encoding="utf-8")


def _revert(backup):
    for f, old in backup.items():
        _write(f, old)


def reasoner_rounds(log) -> tuple[bool, str]:
    """Nivel 2: dos intentos con deepseek-reasoner. Una instruccion por archivo."""
    for i in range(REASONER_TRIES):
        written = _all_code()
        pick = llm(WRITER, 'Sos un diagnosticador. Respondes SOLO JSON: {"files": [paths], "instructions": {path: "una instruccion"}}. Una instruccion por archivo.',
                   f"Los tests de aceptacion fallan. Diagnostica y indica QUE cambiar.\n\nSALIDA:\n{log}\n\n"
                   f"TAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
        write_supervision(f"reasoner {i+1}: {pick[:2000]}")
        try:
            data = json.loads(strip_fence(pick))
            files = [f for f in data.get("files", []) if f in written]
            instr = data.get("instructions") or {}
        except Exception:
            files, instr = list(written)[:1], {}
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Aplica ESTA instruccion a {f}. No toques tests_accept.\n"
                      f"INSTRUCCION: {instr.get(f, 'corregi el fallo del test')}\n\n"
                      f"SALIDA:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code:
                _write(f, code)
        if not gate_baseline()[0] or not gate_compile(files)[0] or not gate_test_compile()[0]:
            _revert(backup)
            continue
        ok, log = accept()
        state["reasoner_rounds"] = state.get("reasoner_rounds", []) + [{"i": i + 1, "files": files, "ok": ok}]
        if ok:
            state["suite_total"] = test_counts(log)
            return True, f"reasoner {i+1} verde"
    return False, f"reasoner x{REASONER_TRIES} rojo"


def claude_fix(log) -> tuple[bool, str]:
    """Nivel 3: Claude en el worktree (modo edit). Deja correccion; el test de aceptacion decide."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)  # ponytail: el CLI no anida dentro de una sesion Claude Code
    state["escalated_to_claude"] = True
    state["claude_calls"] += 1
    r = run_claude(f"Los tests de aceptacion fallan. Arregla el codigo en {state['plan_files']} hasta que "
                   f"`pytest tests_accept -q` pase. NO toques tests_accept ni docs. Al final responde en una linea que cambiaste.\n\n"
                   f"TAREA:\n{TASK.task}\n\nSALIDA:\n{log}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"nivel 3 Claude (rc={r.get('returncode')}): {(r.get('result') or '')[:2000]}")
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    ok, log = accept()
    if ok:
        state["suite_total"] = test_counts(log)
    return ok, "Claude verde" if ok else "Claude rojo: escala a humano"


def self_check() -> int:
    """Cero API: allowlist, novedad, conteo pytest."""
    fails = []
    good = "## Archivos\n- `limiter/core.py`\n- `limiter/multi.py`\n- `limiter/__init__.py`\n\n## Prueba\npytest\n"
    if not gate_plan_allowlist(good, ["limiter/core.py", "limiter/multi.py", "limiter/__init__.py"])[0]:
        fails.append("allowlist-good")
    if gate_plan_allowlist("## Archivos\n- tests_accept/test_limiter.py\n- limiter/core.py", ["limiter/core.py"])[0]:
        fails.append("allowlist-tests")
    if gate_plan_allowlist(good, ["limiter/core.py"])[0]:
        fails.append("allowlist-omite")
    SEEN.clear()
    if _novelty({"A": "x\ny"}, {"A": "x\nz"}) != 1.0 or _novelty({"A": "x\nz"}, {"A": "x\ny"}) != 0.0:
        fails.append("novelty")
    if test_counts("2 failed, 1 passed in 0.1s") != {"run": 3, "failed": 2} or test_counts("3 passed in 0.1s") != {"run": 3, "failed": 0}:
        fails.append("counts")
    print("self-check", "FAIL" if fails else "PASS", fails)
    return 1 if fails else 0


@stage("2-spec")
def spec():
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.",
              f"Escribi spec.md: requisitos y diseño para implementar esta tarea en Python 3.12, sin dependencias.\n\n"
              f"TAREA:\n{TASK.task}\n\nTESTS DE ACEPTACION (no se modifican):\n{_tests_text()}\n\n"
              "La spec tiene que: (1) listar cada archivo, clase y metodo con firmas y tipos exactos; "
              "(2) describir cada regla de comportamiento con el orden de evaluacion y los casos borde (recarga fraccionaria, cap, keys nuevas); "
              "(3) nombrar los archivos con su path exacto. Sin 'TBD'. Solo markdown.")
    _write("docs/sdlc/spec.md", out)
    missing = [s for s in CONTRACT if s not in out]
    if missing or "TBD" in out:
        out = llm(WRITER, "Sos un ingeniero. Reescribís la spec. Los tokens pedidos deben aparecer VERBATIM.",
                  f"Esta spec fallo el gate. Inserta estos tokens verbatim: {missing}. No borres el resto. Sin TBD.\n\nSPEC ACTUAL:\n{out}")
        _write("docs/sdlc/spec.md", out)
        missing = [s for s in CONTRACT if s not in out]
    if missing or "TBD" in out:
        rec_gate("G1-spec-contrato", False, f"faltan {missing}")
        return False, f"G1 rechaza: faltan {missing}"
    rec_gate("G1-spec-contrato", True, "contrato cubierto, sin TBD")
    return True, "G1 ok"


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    out = llm(WRITER, "Sos un tech lead. Escribis planes ejecutables. NO regeneres tests_accept.",
              f"Escribi plan.md. Formato OBLIGATORIO: seccion '## Archivos' con un item `path.py` por archivo a escribir, "
              f"en orden de dependencia (primero los que no importan a otros). '## Prueba' = `python -m pytest tests_accept -q`.\n\nSPEC:\n{spec_md}")
    _write("docs/sdlc/plan.md", out)
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", out, re.S | re.I)
    files = list(dict.fromkeys(re.findall(PY_RE, m.group(1) if m else out)))
    files = [f for f in files if not f.startswith("tests_accept/")]
    state["plan_files"] = files
    return gate_plan_allowlist(out, files)


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    written = {}
    for f in state["plan_files"]:
        out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi {f}. Tiene que importar con los archivos ya escritos y pasar los tests de aceptacion.\n\n"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nTAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\n"
                  f"ARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = strip_fence(out)
        _write(f, code)
        written[f] = code
        bok, bnote = gate_baseline()
        if not bok:
            return False, bnote
    ok, log = gate_compile(state["plan_files"])
    for _ in range(3):
        if ok:
            break
        for f in state["plan_files"]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo corregido del archivo.",
                      f"py_compile fallo. Corregi {f}.\n\nERROR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code and code != written[f]:
                _write(f, code); written[f] = code
        ok, log = gate_compile(state["plan_files"])
        if not gate_baseline()[0]:
            return False, "baseline roto en compile-fix"
    if not ok:
        return False, "G3 rechaza: no compila"
    tok, tnote = gate_test_compile()
    if not tok:
        return False, tnote
    return True, "G3 + test-compile ok"


@stage("5-test")
def test():
    ok, log = accept()
    state["test_rounds"] = state.get("test_rounds", []) + [{"round": 0, "tests": test_counts(log)}]
    state["stall"] = 0
    vueltas = 0
    while not ok and vueltas < MAX_FIX:
        vueltas += 1
        written = _all_code()
        pick = llm(CODER, 'Sos un programador Python senior. Respondes SOLO un JSON: {"files": [paths], "why": str}.',
                   f"Los tests fallan. Que archivos cambiar (los MENOS posibles)?\n\nSALIDA:\n{log}\n\n"
                   f"TAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
        try:
            files = [f for f in json.loads(strip_fence(pick)).get("files", []) if f in written]
        except Exception:
            files = state["plan_files"][:1]
        if not files:
            files = state["plan_files"][:1]
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero. No toques tests_accept.",
                      f"Arregla {f}. Los tests NO se modifican.\n\nSALIDA:\n{log}\n\nTAREA:\n{TASK.task}\n\n"
                      f"TESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code and code != written[f]:
                _write(f, code)
        if not gate_baseline()[0] or not gate_compile(files)[0] or not gate_test_compile()[0]:
            _revert(backup)
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "gate"})
            continue
        prev = state["test_rounds"][-1].get("tests") or {"failed": 10 ** 6}
        ok, log2 = accept()
        now = test_counts(log2) or {"failed": 10 ** 6, "run": 0}
        nov = _novelty(backup, {f: (WT / f).read_text(encoding="utf-8") for f in files})
        state["stall"] = state["stall"] + 1 if now["failed"] >= prev["failed"] else 0
        # Gate de REGRESION por unidad (ticket 13): si la vuelta hace fallar mas tests, se revierte.
        if now["failed"] > prev["failed"]:
            _revert(backup)
            rec_gate("regresion-unidad", False, f"{files} rompio {now['failed'] - prev['failed']} test(s) que pasaban")
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "regresion", "tests": now, "novelty": nov})
        else:
            rec_gate("regresion-unidad", True, f"{files}: {prev['failed']} -> {now['failed']} fallos")
            ok, log = ok, log2
            state["test_rounds"].append({"round": vueltas, "files": files, "tests": now, "novelty": nov})
        # Topes por AVANCE (ticket 07): sin bajar fallos N vueltas, o diff sin novedad -> escala un nivel.
        if not ok and (state["stall"] >= CFG["stall_rounds"] or nov < CFG["diff_novelty_min"]):
            rec_gate("avance", False, f"stall={state['stall']} novedad={nov}: escala a reasoner")
            break
    state["test_fix_rounds"] = state.get("test_fix_rounds", 0) + vueltas
    if ok:
        state["suite_total"] = test_counts(log)
        rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas")
        return True, f"G4 ok ({vueltas} vueltas)"
    rok, rnote = reasoner_rounds(log)
    if rok:
        rec_gate("G4-aceptacion", True, rnote)
        return True, rnote
    cok, cnote = claude_fix(log)
    rec_gate("G4-aceptacion", cok, cnote)
    if not cok:
        write_supervision("ESCALATE_HUMAN: fix 3 + reasoner 2 + Claude agotados.")
    return cok, cnote


@stage("5b-review")
def review():
    """Claude revisa el diff. Bloquea SOLO si deja un test_review.py que falla (ticket 07)."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    diff = subprocess.run(["git", "diff", "--cached", "--", *state["plan_files"]], cwd=WT,
                          capture_output=True, text=True, encoding="utf-8").stdout
    state["claude_calls"] += 1
    r = run_claude(
        "Revisa este diff contra docs/sdlc/spec.md y la TAREA. "
        f"Si encontras un defecto REAL, escribi UN test pytest que falle en {REVIEW_REL} "
        "(una funcion test_*) y responde 'BLOCK: <defecto>'. No toques ningun otro archivo. "
        "Si no hay defecto demostrable con test, no escribas nada y responde 'OK' o 'NOTE: <observacion>'.\n\n"
        f"TAREA:\n{TASK.task}\n\nDIFF:\n{diff[:60000]}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    verdict = (r.get("result") or "").strip()
    write_supervision(f"revision del diff (Claude, rc={r.get('returncode')}): {verdict[:2000]}")
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    if not (WT / REVIEW_REL).exists():
        return rec_gate("claude-diff-review", True, f"sin test nuevo: {verdict[:120]}")
    SNAP[REVIEW_REL] = sha_file(REVIEW_REL)
    ok, _ = accept()
    if ok:
        return rec_gate("claude-diff-review", True, "test_review pasa: sin evidencia, PR sigue")
    state["review_block"] = True
    rec_gate("claude-diff-review", False, f"test_review falla: vuelve a build. {verdict[:120]}")
    return True, "bloqueo con evidencia"


@stage("6-pr")
def pr():
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m",
                    f"sdlc: {TASK_NAME} — driver_py, gates + escalera + revision Claude"], cwd=WT, check=True)
    ds = subprocess.run(["git", "diff", "--stat", "HEAD~1"], cwd=WT, capture_output=True, text=True).stdout
    state["diffstat"] = ds.strip().splitlines()[-1] if ds.strip() else ""
    ns = subprocess.run(["git", "diff", "--numstat", "HEAD~1"], cwd=WT, capture_output=True, text=True).stdout
    rows = [l.split("\t") for l in ns.splitlines() if l.count("\t") == 2 and l.split("\t")[0].isdigit()]
    state["lines"] = {"added": sum(int(a) for a, _, _ in rows), "deleted": sum(int(d) for _, d, _ in rows)}
    pkg = sorted({f.split("/")[0] for f in state["plan_files"] if "/" in f}) or state["plan_files"]
    _, rl = sh([PY, "-m", "ruff", "check", "--output-format", "concise", *pkg])
    _, ml = sh([PY, "-m", "mypy", "--ignore-missing-imports", *pkg])
    state["lint_new"] = {"ruff": len(re.findall(r"^\S+:\d+:\d+: ", rl, re.M)),
                         "mypy": int((re.search(r"Found (\d+) error", ml) or [0, 0])[1])}
    state["mutation_score"] = None  # ponytail: mutmut queda para el ticket 13
    return True, state["diffstat"]


if __name__ == "__main__":
    TASK = bench.get_task(TASK_NAME)
    CONTRACT = CONTRACTS.get(TASK_NAME, [])
    if "--self-check" in sys.argv:
        sys.exit(self_check())
    if not (WT / ".git").exists():
        WT.parent.mkdir(parents=True, exist_ok=True)
        bench.materialize(TASK, str(WT))
        print("materializado", WT, flush=True)
    snapshot_baseline()
    state["t0"] = time.strftime("%Y-%m-%d %H:%M:%S")
    state["t0_epoch"] = time.time()
    from_stage = float(_arg("--from-stage", "2"))
    if from_stage > 3:
        plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
        state["plan_files"] = [f for f in dict.fromkeys(re.findall(PY_RE, plan_md)) if not f.startswith("tests_accept/")]
    stages = {2: spec, 3: plan, 4: build, 5: test, 5.5: review, 6: pr}
    for k in sorted(stages):
        if k >= from_stage:
            stages[k]()
            if k == 5.5 and state.get("review_block"):
                test()  # una vuelta mas con el test de Claude; si sigue rojo, escala
    state["minutes_total"] = round((time.time() - state["t0_epoch"]) / 60, 2)
    state["usd"] = ledger_usd()
    _flush()
    here = pathlib.Path(__file__).resolve().parent / f"run-log-{PHASE}.json"
    here.write_text(LOG.read_text(encoding="utf-8"), encoding="utf-8")
    print("DRIVER_PY TERMINO", json.dumps({k: state.get(k) for k in
          ("task", "calls", "minutes_total", "usd", "usd_by_family", "human_interventions", "claude_calls", "review_block",
           "escalated_to_claude", "gate_rejects", "diffstat", "lines", "suite_total", "lint_new")}, ensure_ascii=False))
