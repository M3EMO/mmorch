"""Brazo B del A/B: SDLC de 6 etapas "a mano", orquestado por script, ejecutado por mmorch.

Etapas y gates (cada gate es determinista o un comando; ningun juez LLM):
  1 intent.md   -> lo escribio el humano (ya existe).
  2 spec.md     <- deepseek-reasoner.  G1: cubre todos los simbolos del contrato, sin TBD.
  3 plan.md     <- deepseek-reasoner.  G2: lista >=3 .java bajo com/qtp/bot, incluye Catalog y Bot.
  4 build       <- deepseek-v4-pro, un archivo por llamada segun el plan.  G3: mvn compile.
  5 test        <- G4: mvn test (aceptacion). Loop de fix v2 con v4-pro, max 3 vueltas:
                   el modelo nombra QUE archivos tocar, gate de compile por vuelta con REVERT
                   si la vuelta rompe la compilacion, conteo de tests por vuelta.
  6 PR          -> commit en ab/b-sdlc con diffstat y resumen.
Cada llamada lleva phase="ab-sdlc-B" -> el costo se lee de logs/metrics.jsonl por fase.
Se registra: minutos por etapa, llamadas, intervenciones humanas, rechazos de gate, y las
correcciones al ARNES (aparte del proceso).

Historia: v1 (2026-09-10 14:12) fallo en la etapa 5 -- reescribia los 9 archivos por vuelta
y en la vuelta 3 rompio la compilacion (stockAnswer con 3 args vs 4): 27 llamadas, 21 min,
$1.3 en total. v2 = fix dirigido + compile gate + revert.

Uso: python ab_sdlc_b.py [--from-stage N]   (N=4 retoma con spec.md y plan.md existentes)
"""
from __future__ import annotations

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


# --wt / --phase / --max-fix: el mismo driver sirve para el brazo C (hibrido): otro worktree,
# otra fase en el ledger, y max-fix 0 porque ahi los fixes los decide Claude, no el loop.
WT = pathlib.Path(_arg("--wt", r"C:\Users\map12\Desktop\QueTePario\ChatBot-abB"))
BACK = WT / "backend"
SRC = BACK / "src" / "main" / "java" / "com" / "qtp" / "bot"
MVN = r"C:\Users\map12\.claude\tools\apache-maven-3.9.9\bin\mvn.cmd"
PHASE = _arg("--phase", "ab-sdlc-B")
MAX_FIX = int(_arg("--max-fix", "3"))
WRITER, CODER = "deepseek-reasoner", "deepseek-v4-pro"
LOG = WT / "docs" / "sdlc" / "run-log.json"
JAVA_RE = r"src/main/java/com/qtp/bot/(\w+)\.java"

CONTRACT = ["Catalog", "load", "Product", "Variant", "Bot", "reply", "Reply", "replies",
            "paused", "order", "Msg", "text", "image", "MENU", "HANDOFF", "MAYORISTA"]

state: dict = {"stages": [], "calls": 0, "human_interventions": 0, "gate_rejects": [],
               "harness_fixes": ["max_tokens 16384->32768, timeout 240->400 (v1: reasoner agoto el budget en la spec)",
                                 "fix loop v2: archivos dirigidos + compile gate con revert + conteo por vuelta (v1 se rompio a si mismo)"]}


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
             pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
    return r.text


def stage(name):
    def deco(fn):
        def run():
            t0 = time.time(); c0 = state["calls"]
            ok, note = fn()
            state["stages"].append({"stage": name, "ok": ok, "minutes": round((time.time() - t0) / 60, 2),
                                    "calls": state["calls"] - c0, "note": note})
            LOG.write_text(json.dumps(state, indent=1, ensure_ascii=False), encoding="utf-8")
            print(f"[{name}] ok={ok} {note} ({state['calls'] - c0} llamadas, {(time.time() - t0) / 60:.1f} min)", flush=True)
            if not ok:
                sys.exit(f"etapa {name} fallo: {note}")
        return run
    return deco


def strip_fence(t):
    m = re.search(r"```(?:\w+)?\s*(.*?)```", t, re.S)
    return (m.group(1) if m else t).strip()


def mvn(goal):
    p = subprocess.run([MVN, "-q", "-B", goal], cwd=BACK, capture_output=True, text=True, timeout=600)
    return p.returncode == 0, (p.stdout + p.stderr)[-6000:]


def test_counts(log):
    m = re.findall(r"Tests run: (\d+), Failures: (\d+), Errors: (\d+)", log)
    if not m:
        return None
    return {"run": sum(int(x[0]) for x in m), "failed": sum(int(x[1]) + int(x[2]) for x in m)}


INTENT = (WT / "docs/sdlc/intent.md").read_text(encoding="utf-8")
DEMO = (BACK / "reference/demo.py").read_text(encoding="utf-8")
TEST = (BACK / "src/test/java/com/qtp/bot/BotAcceptanceTest.java").read_text(encoding="utf-8")


@stage("2-spec")
def spec():
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.",
              f"Escribi spec.md: requisitos y diseño para implementar este intent en Java 17.\n\n"
              f"INTENT:\n{INTENT}\n\nORACULO demo.py:\n```python\n{DEMO}\n```\n\nTEST DE ACEPTACION (no se modifica):\n```java\n{TEST}\n```\n\n"
              "La spec tiene que: (1) listar cada clase del contrato con sus metodos y tipos exactos; "
              "(2) describir cada regla de comportamiento de demo.py con el orden de evaluacion; "
              "(3) listar los textos literales (MENU, SALUDO, HORARIO, ENVIOS, MAYORISTA, HANDOFF, NOENTIENDO, VOLVER) tal cual; "
              "(4) señalar las trampas del port (normalizacion NFD, difflib ratio, regex con \\b sobre texto normalizado, formato de precio ARS con punto de miles, emojis). "
              "Sin 'TBD'. Solo markdown.")
    (WT / "docs/sdlc/spec.md").write_text(out, encoding="utf-8")
    missing = [s for s in CONTRACT if s not in out]
    if missing or "TBD" in out:
        state["gate_rejects"].append({"gate": "G1", "missing": missing, "tbd": "TBD" in out})
        return False, f"G1 rechaza: faltan {missing}"
    return True, "G1 ok: contrato cubierto, sin TBD"


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    out = llm(WRITER, "Sos un tech lead. Escribis planes de implementacion ejecutables por otro agente sin mas contexto.",
              f"Escribi plan.md a partir de esta spec. Formato OBLIGATORIO: una seccion '## Archivos' con una lista, "
              f"un item por archivo, con la ruta relativa a backend/ (src/main/java/com/qtp/bot/X.java) y una linea de responsabilidad; "
              f"orden de construccion; '## Riesgos'; '## Prueba' = `cd backend && mvn -q -B test`. Jackson y JUnit ya estan en el pom.\n\nSPEC:\n{spec_md}")
    (WT / "docs/sdlc/plan.md").write_text(out, encoding="utf-8")
    files = list(dict.fromkeys(re.findall(JAVA_RE, out)))
    state["plan_files"] = files
    if len(files) < 3 or "Catalog" not in files or "Bot" not in files:
        state["gate_rejects"].append({"gate": "G2", "files": files})
        return False, f"G2 rechaza: archivos {files}"
    return True, f"G2 ok: {files}"


def _all_code():
    return {f: (SRC / f"{f}.java").read_text(encoding="utf-8") for f in state["plan_files"] if (SRC / f"{f}.java").exists()}


def _joined(written):
    return "\n\n".join(f"// {k}.java\n{v}" for k, v in written.items())


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    SRC.mkdir(parents=True, exist_ok=True)
    written = {}
    for f in state["plan_files"]:
        out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi src/main/java/com/qtp/bot/{f}.java segun el plan y la spec. Paquete com.qtp.bot. "
                  f"Tiene que compilar con los archivos ya escritos (abajo) y pasar el test de aceptacion.\n\n"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nORACULO demo.py:\n```python\n{DEMO}\n```\n\n"
                  f"TEST:\n```java\n{TEST}\n```\n\nARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = strip_fence(out)
        (SRC / f"{f}.java").write_text(code, encoding="utf-8")
        written[f] = code
    ok, log = mvn("compile")
    for _ in range(3):
        if ok:
            break
        for f in state["plan_files"]:
            out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el codigo corregido del archivo, sin explicacion ni markdown.",
                      f"mvn compile fallo. Corregi src/main/java/com/qtp/bot/{f}.java (devolvelo entero) para que compile con los demas.\n\n"
                      f"ERROR:\n{log}\n\nARCHIVOS ACTUALES:\n{_joined(written)}")
            code = strip_fence(out)
            if code and code != written[f]:
                (SRC / f"{f}.java").write_text(code, encoding="utf-8"); written[f] = code
        ok, log = mvn("compile")
    if not ok:
        state["gate_rejects"].append({"gate": "G3", "log": log[-1500:]})
        return False, "G3 rechaza: no compila tras 3 vueltas"
    return True, "G3 ok: compila"


@stage("5-test")
def test():
    ok, log = mvn("test")
    state["test_rounds"] = [{"round": 0, "compiles": "COMPILATION ERROR" not in log, "tests": test_counts(log)}]
    vueltas = 0
    while not ok and vueltas < MAX_FIX:
        vueltas += 1
        written = _all_code()
        allcode = _joined(written)
        pick = llm(CODER, 'Sos un programador Java 17 senior. Respondes SOLO un JSON: {"files": [nombres sin .java], "why": str}.',
                   f"El test de aceptacion fallo. Que archivos hay que cambiar (los MENOS posibles)?\n\nSALIDA:\n{log}\n\n"
                   f"ORACULO:\n```python\n{DEMO}\n```\n\nTEST:\n```java\n{TEST}\n```\n\nARCHIVOS:\n{allcode}")
        try:
            files = [f for f in json.loads(strip_fence(pick)).get("files", []) if f in written]
        except Exception:
            files = ["Bot"] if "Bot" in written else state["plan_files"][:1]
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Java 17 senior. Devolves SOLO el codigo corregido del archivo entero, sin explicacion ni markdown.",
                      f"Arregla src/main/java/com/qtp/bot/{f}.java para que pase el test. El oraculo es demo.py; el test NO se modifica. "
                      f"No cambies firmas que otros archivos usan.\n\nSALIDA DEL TEST:\n{log}\n\nORACULO:\n```python\n{DEMO}\n```\n\n"
                      f"TEST:\n```java\n{TEST}\n```\n\nARCHIVOS:\n{allcode}")
            code = strip_fence(out)
            if code and code != written[f]:
                (SRC / f"{f}.java").write_text(code, encoding="utf-8")
        cok, clog = mvn("compile")
        if not cok:
            for f, old in backup.items():
                (SRC / f"{f}.java").write_text(old, encoding="utf-8")
            state["test_rounds"].append({"round": vueltas, "files": files, "compiles": False, "reverted": True})
            continue
        ok, log = mvn("test")
        state["test_rounds"].append({"round": vueltas, "files": files, "compiles": True, "tests": test_counts(log)})
    state["test_fix_rounds"] = vueltas
    if not ok:
        state["gate_rejects"].append({"gate": "G4", "log": log[-2000:]})
        return False, f"G4 rechaza: test rojo tras {vueltas} vueltas ({state['test_rounds'][-1].get('tests')})"
    return True, f"G4 ok: aceptacion verde ({vueltas} vueltas de fix)"


@stage("6-pr")
def pr():
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    msg = "B: motor del agente en Java (port de demo.py) -- SDLC 6 etapas, gates deterministas"
    subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m", msg], cwd=WT, check=True)
    ds = subprocess.run(["git", "diff", "--stat", "HEAD~1"], cwd=WT, capture_output=True, text=True).stdout
    state["diffstat"] = ds.strip().splitlines()[-1] if ds.strip() else ""
    return True, state["diffstat"]


if __name__ == "__main__":
    state["t0"] = time.strftime("%Y-%m-%d %H:%M:%S"); state["t0_epoch"] = time.time()
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
    LOG.write_text(json.dumps(state, indent=1, ensure_ascii=False), encoding="utf-8")
    print("B TERMINO", json.dumps({k: state.get(k) for k in ("calls", "minutes_total", "human_interventions", "gate_rejects", "diffstat", "test_rounds")}, ensure_ascii=False))
