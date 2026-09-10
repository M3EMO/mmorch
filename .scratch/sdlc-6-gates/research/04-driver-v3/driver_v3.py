"""PROTOTIPO throwaway — driver v3 (ticket 04). No es el engine vivo.

B v2 + 3 gates A/B (plan-allowlist, baseline, test-compile; firmas-vs-test se saco: mvn test-compile lo cubre)
+ regresion por unidad en el fix loop + escalera 3 vueltas / reasoner×2 / stop (no llama Claude).
Sin pistas del bug de ayer en los prompts: los gates tienen que atraparlo solos.
"""
from __future__ import annotations

import hashlib
import json
import pathlib
import re
import subprocess
import sys
import time

sys.path.insert(0, r"C:\Users\map12\.claude\orchestration")
from mmorch.providers import call  # noqa: E402

def _arg(flag, default):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default


WT = pathlib.Path(_arg("--wt", r"C:\Users\map12\Desktop\QueTePario\ChatBot-abV3"))
BACK = WT / "backend"
SRC = BACK / "src" / "main" / "java" / "com" / "qtp" / "bot"
TEST_REL = "src/test/java/com/qtp/bot/BotAcceptanceTest.java"
POM_REL = "pom.xml"
MVN = r"C:\Users\map12\.claude\tools\apache-maven-3.9.9\bin\mvn.cmd"
PHASE = _arg("--phase", "ab-sdlc-v3")
MAX_FIX = int(_arg("--max-fix", "3"))
REASONER_TRIES = 2
WRITER, CODER = "deepseek-reasoner", "deepseek-v4-pro"
LOG = WT / "docs" / "sdlc" / "run-log.json"
JAVA_RE = r"src/main/java/com/qtp/bot/(\w+)\.java"
METRICS = pathlib.Path(r"C:\Users\map12\.claude\orchestration\logs\metrics.jsonl")

CONTRACT = ["Catalog", "load", "Product", "Variant", "Bot", "reply", "Reply", "replies",
            "paused", "order", "Msg", "text", "image", "MENU", "HANDOFF", "MAYORISTA"]

state: dict = {
    "proto": "driver_v3",
    "stages": [], "gates": [], "calls": 0, "human_interventions": 0,
    "claude_calls": 0, "gate_rejects": [], "escalated_to_claude": False,
}

SNAP: dict[str, str] = {}


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
             pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
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


def mvn(goal):
    # sin -q para "test": el gate de regresion cuenta "Tests run: X, Failures: Y" por clase
    args = [MVN, "-B", goal] if goal == "test" else [MVN, "-q", "-B", goal]
    p = subprocess.run(args, cwd=BACK, capture_output=True, text=True, timeout=600)
    return p.returncode == 0, (p.stdout + p.stderr)[-6000:]


def test_counts(log):
    m = re.findall(r"Tests run: (\d+), Failures: (\d+), Errors: (\d+)", log)
    if not m:
        return None
    return {"run": sum(int(x[0]) for x in m), "failed": sum(int(x[1]) + int(x[2]) for x in m)}


def sha_file(rel):
    return hashlib.sha256((BACK / rel).read_bytes()).hexdigest()


def snapshot_baseline():
    SNAP[POM_REL] = sha_file(POM_REL)
    SNAP[TEST_REL] = sha_file(TEST_REL)


def gate_plan_allowlist(plan_md: str, files: list[str]) -> tuple[bool, str]:
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    block = m.group(1) if m else plan_md
    if re.search(r"(?im)^\s*[-*]+\s*`?pom\.xml`?", block):
        return rec_gate("plan-allowlist", False, "plan pide pom.xml")
    if re.search(r"(?im)^\s*[-*]+\s*`?.*BotAcceptanceTest", block):
        return rec_gate("plan-allowlist", False, "plan lista el test de aceptacion como archivo a escribir")
    if len(files) < 3 or "Catalog" not in files or "Bot" not in files:
        return rec_gate("plan-allowlist", False, f"archivos {files}")
    return rec_gate("plan-allowlist", True, f"{files}")


def gate_baseline() -> tuple[bool, str]:
    bad = [rel for rel, h in SNAP.items() if sha_file(rel) != h]
    return rec_gate("baseline-intacto", not bad, "ok" if not bad else f"toco {bad}")


def gate_test_compile() -> tuple[bool, str]:
    ok, log = mvn("test-compile")
    return rec_gate("test-compile", ok, "ok" if ok else log[-800:])


def ledger_usd():
    if not METRICS.exists():
        return None
    t0 = state.get("t0_epoch") or 0
    usd = 0.0
    for line in METRICS.read_text(encoding="utf-8").splitlines()[-8000:]:
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if r.get("phase") == PHASE and float(r.get("ts") or 0) >= t0:
            usd += float(r.get("cost_usd") or 0)
    return round(usd, 4)


def _joined(written):
    return "\n\n".join(f"// {k}.java\n{v}" for k, v in written.items())


def _all_code():
    return {f: (SRC / f"{f}.java").read_text(encoding="utf-8") for f in state["plan_files"] if (SRC / f"{f}.java").exists()}


def write_supervision(note: str):
    p = WT / "docs" / "sdlc" / "supervision.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    prev = p.read_text(encoding="utf-8") if p.exists() else "# supervision.md — driver v3\n\n"
    p.write_text(prev + f"\n## candidato {time.strftime('%H:%M:%S')}\n{note}\n", encoding="utf-8")


def reasoner_rounds(log, demo, test) -> tuple[bool, str]:
    """Nivel 2: dos intentos. No llama Claude."""
    for i in range(REASONER_TRIES):
        written = _all_code()
        pick = llm(WRITER, 'Sos un diagnosticador. Respondes SOLO JSON: {"files": [nombres], "instructions": {nombre: "una instruccion"}}. Una instruccion por archivo.',
                   f"El oraculo de test fallo. Diagnostica y indica QUE cambiar.\n\nSALIDA:\n{log}\n\n"
                   f"ORACULO demo.py (recorte):\n```python\n{demo[:8000]}\n```\n\nTEST:\n```java\n{test}\n```\n\n"
                   f"ARCHIVOS:\n{_joined(written)}")
        write_supervision(f"reasoner {i+1}: {pick[:2000]}")
        try:
            data = json.loads(strip_fence(pick))
            files = [f for f in data.get("files", []) if f in written]
            instr = data.get("instructions") or {}
        except Exception:
            files, instr = (["Catalog"] if "Catalog" in written else list(written)[:1]), {}
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el archivo entero.",
                      f"Aplica ESTA instruccion a {f}.java. No toques el test ni el pom.\n"
                      f"INSTRUCCION: {instr.get(f, 'corregi el fallo del test')}\n\n"
                      f"SALIDA:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code:
                (SRC / f"{f}.java").write_text(code, encoding="utf-8")
        bok, _ = gate_baseline()
        if not bok:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            continue
        cok, clog = gate_test_compile()
        if not cok:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            log = clog
            continue
        ok, log = mvn("test")
        state["reasoner_rounds"] = state.get("reasoner_rounds", []) + [{"i": i + 1, "files": files, "ok": ok}]
        if ok:
            return True, f"reasoner {i+1} verde"
    return False, f"reasoner x{REASONER_TRIES} rojo"


def self_check() -> int:
    """Cero API. Los 4 gates con fixtures."""
    fails = []
    ok, _ = gate_plan_allowlist("## Archivos\n- `src/main/java/com/qtp/bot/Catalog.java`\n- `src/main/java/com/qtp/bot/Bot.java`\n- `src/main/java/com/qtp/bot/Msg.java`\n\n## Prueba\nBotAcceptanceTest no se toca\n",
                                ["Catalog", "Bot", "Msg"])
    if not ok:
        fails.append("allowlist-good")
    ok, _ = gate_plan_allowlist("- pom.xml\n- src/main/java/com/qtp/bot/Bot.java", ["Bot"])
    if ok:
        fails.append("allowlist-pom")
    print("self-check", "FAIL" if fails else "PASS", fails)
    return 1 if fails else 0


@stage("2-spec")
def spec():
    intent = (WT / "docs/sdlc/intent.md").read_text(encoding="utf-8")
    demo = (BACK / "reference/demo.py").read_text(encoding="utf-8")
    test = (BACK / TEST_REL).read_text(encoding="utf-8")
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.",
              f"Escribi spec.md: requisitos y diseño para implementar este intent en Java 17.\n\n"
              f"INTENT:\n{intent}\n\nORACULO demo.py:\n```python\n{demo}\n```\n\nTEST DE ACEPTACION (no se modifica):\n```java\n{test}\n```\n\n"
              "La spec tiene que: (1) listar cada clase del contrato con sus metodos y tipos exactos; "
              "(2) describir cada regla de comportamiento de demo.py con el orden de evaluacion; "
              "(3) listar los textos literales (MENU, SALUDO, HORARIO, ENVIOS, MAYORISTA, HANDOFF, NOENTIENDO, VOLVER) tal cual; "
              "(4) señalar las trampas del port (normalizacion NFD, difflib ratio, regex con \\b sobre texto normalizado, formato de precio ARS con punto de miles, emojis). "
              "Sin 'TBD'. Solo markdown.")
    (WT / "docs/sdlc/spec.md").write_text(out, encoding="utf-8")
    missing = [s for s in CONTRACT if s not in out]
    if missing or "TBD" in out:
        out = llm(WRITER, "Sos un ingeniero. Reescribís la spec. Los tokens pedidos deben aparecer VERBATIM.",
                  f"Esta spec fallo el gate. Inserta estos tokens verbatim (como aparecen aca): {missing}. "
                  f"No borres el resto. Sin TBD.\n\nSPEC ACTUAL:\n{out}")
        (WT / "docs/sdlc/spec.md").write_text(out, encoding="utf-8")
        missing = [s for s in CONTRACT if s not in out]
    if missing or "TBD" in out:
        rec_gate("G1-spec-contrato", False, f"faltan {missing}")
        return False, f"G1 rechaza: faltan {missing}"
    rec_gate("G1-spec-contrato", True, "contrato cubierto, sin TBD")
    return True, "G1 ok"


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    out = llm(WRITER, "Sos un tech lead. Escribis planes ejecutables. NO regeneres pom.xml ni el test de aceptacion.",
              f"Escribi plan.md. Formato OBLIGATORIO: seccion '## Archivos' con items "
              f"`src/main/java/com/qtp/bot/X.java` solamente. Jackson y JUnit ya estan en el pom. "
              f"'## Prueba' = `cd backend && mvn -q -B test`.\n\nSPEC:\n{spec_md}")
    (WT / "docs/sdlc/plan.md").write_text(out, encoding="utf-8")
    files = list(dict.fromkeys(re.findall(JAVA_RE, out)))
    state["plan_files"] = files
    ok, note = gate_plan_allowlist(out, files)
    return ok, note


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    demo = (BACK / "reference/demo.py").read_text(encoding="utf-8")
    test = (BACK / TEST_REL).read_text(encoding="utf-8")
    SRC.mkdir(parents=True, exist_ok=True)
    written = {}
    for f in state["plan_files"]:
        out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi src/main/java/com/qtp/bot/{f}.java. Paquete com.qtp.bot. "
                  f"Tiene que compilar con los archivos ya escritos y pasar el test de aceptacion.\n\n"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nORACULO demo.py:\n```python\n{demo}\n```\n\n"
                  f"TEST:\n```java\n{test}\n```\n\nARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = strip_fence(out)
        (SRC / f"{f}.java").write_text(code, encoding="utf-8")
        written[f] = code
        bok, bnote = gate_baseline()
        if not bok:
            return False, bnote
    ok, log = mvn("compile")
    for _ in range(3):
        if ok:
            break
        for f in state["plan_files"]:
            out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el codigo corregido del archivo.",
                      f"mvn compile fallo. Corregi {f}.java.\n\nERROR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code and code != written[f]:
                (SRC / f"{f}.java").write_text(code, encoding="utf-8"); written[f] = code
        ok, log = mvn("compile")
        bok, _ = gate_baseline()
        if not bok:
            return False, "baseline roto en compile-fix"
    if not ok:
        rec_gate("G3-compile", False, log[-1500:])
        return False, "G3 rechaza: no compila"
    rec_gate("G3-compile", True, "compila")
    tok, tnote = gate_test_compile()
    if not tok:
        return False, tnote
    return True, "G3 + test-compile ok"


@stage("5-test")
def test():
    demo = (BACK / "reference/demo.py").read_text(encoding="utf-8")
    test = (BACK / TEST_REL).read_text(encoding="utf-8")
    ok, log = mvn("test")
    state["test_rounds"] = [{"round": 0, "tests": test_counts(log)}]
    vueltas = 0
    while not ok and vueltas < MAX_FIX:
        vueltas += 1
        written = _all_code()
        pick = llm(CODER, 'Sos un programador Java 17 senior. Respondes SOLO un JSON: {"files": [nombres sin .java], "why": str}.',
                   f"El test fallo. Que archivos cambiar (los MENOS posibles)?\n\nSALIDA:\n{log}\n\n"
                   f"ORACULO:\n```python\n{demo}\n```\n\nTEST:\n```java\n{test}\n```\n\nARCHIVOS:\n{_joined(written)}")
        try:
            files = [f for f in json.loads(strip_fence(pick)).get("files", []) if f in written]
        except Exception:
            files = ["Bot"] if "Bot" in written else state["plan_files"][:1]
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el archivo entero. No toques el test.",
                      f"Arregla {f}.java. El test NO se modifica.\n\nSALIDA:\n{log}\n\nORACULO:\n```python\n{demo}\n```\n\n"
                      f"TEST:\n```java\n{test}\n```\n\nARCHIVOS:\n{_joined(written)}")
            code = strip_fence(out)
            if code and code != written[f]:
                (SRC / f"{f}.java").write_text(code, encoding="utf-8")
        bok, _ = gate_baseline()
        if not bok:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "baseline"})
            continue
        cok, clog = gate_test_compile()
        if not cok:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "test-compile"})
            continue
        prev = state["test_rounds"][-1].get("tests") or {"failed": 10 ** 6}
        ok, log2 = mvn("test")
        now = test_counts(log2) or {"failed": 10 ** 6, "run": 0}
        # Gate de REGRESION por unidad (ticket 13, la parte barata): si la vuelta hace fallar
        # mas tests que antes, se revierte y se atribuye a los archivos tocados.
        if now["failed"] > prev["failed"]:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            rec_gate("regresion-unidad", False, f"{files} rompio {now['failed'] - prev['failed']} test(s) que pasaban")
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "regresion", "tests": now})
            continue
        rec_gate("regresion-unidad", True, f"{files}: {prev['failed']} -> {now['failed']} fallos")
        ok, log = ok, log2
        state["test_rounds"].append({"round": vueltas, "files": files, "tests": now})
    state["test_fix_rounds"] = vueltas
    if ok:
        rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas")
        return True, f"G4 ok ({vueltas} vueltas)"
    rok, rnote = reasoner_rounds(log, demo, test)
    if rok:
        rec_gate("G4-aceptacion", True, rnote)
        return True, rnote
    rec_gate("G4-aceptacion", False, rnote)
    state["escalated_to_claude"] = True
    write_supervision("ESCALATE_CLAUDE: tope 3+2 agotado. No se llamo Claude en este prototipo.")
    return False, "escala a Claude (no llamado)"


@stage("6-pr")
def pr():
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    msg = "v3: port demo.py — 4 gates + escalera 3+2, sin Claude"
    subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m", msg], cwd=WT, check=True)
    ds = subprocess.run(["git", "diff", "--stat", "HEAD~1"], cwd=WT, capture_output=True, text=True).stdout
    state["diffstat"] = ds.strip().splitlines()[-1] if ds.strip() else ""
    return True, state["diffstat"]


if __name__ == "__main__":
    if "--self-check" in sys.argv:
        sys.exit(self_check())
    if not (BACK / TEST_REL).exists():
        sys.exit(f"worktree incompleto: {BACK / TEST_REL}")
    snapshot_baseline()
    state["t0"] = time.strftime("%Y-%m-%d %H:%M:%S")
    state["t0_epoch"] = time.time()
    from_stage = int(sys.argv[sys.argv.index("--from-stage") + 1]) if "--from-stage" in sys.argv else 2
    if from_stage > 3:
        plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
        state["plan_files"] = list(dict.fromkeys(re.findall(JAVA_RE, plan_md)))
        state["resumed_from_stage"] = from_stage
    stages = {2: spec, 3: plan, 4: build, 5: test, 6: pr}
    for k in sorted(stages):
        if k >= from_stage:
            stages[k]()
    state["minutes_total"] = round((time.time() - state["t0_epoch"]) / 60, 2)
    state["usd"] = ledger_usd()
    _flush()
    here = pathlib.Path(__file__).resolve().parent / "run-log.json"
    here.write_text(LOG.read_text(encoding="utf-8"), encoding="utf-8")
    print("V3 TERMINO", json.dumps({k: state.get(k) for k in
          ("calls", "minutes_total", "usd", "human_interventions", "claude_calls",
           "escalated_to_claude", "gate_rejects", "diffstat")}, ensure_ascii=False))
