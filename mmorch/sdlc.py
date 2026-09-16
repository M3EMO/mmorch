"""sdlc — pipeline de 6 etapas con gates (spec -> plan -> build -> test -> review -> pr). Ticket 05, paso 1.

Reemplaza a project_integrate.build_project como engine de /project. Validado 6/6 en el bench y D2..D15 sobre
mmorch (research/07-resultados.md del mapa sdlc-6-gates). Gates deterministas (contrato, allowlist, trazabilidad,
compile, test-compile, cambio-minimo, sin-clones, docstring, suite total por nombre, lint, alcance) y escalera
fix loop 3 -> reasoner x2 -> Claude (edit) -> humano. La etapa es el checkpoint: run-log.json + from_stage.

Uso como biblioteca: `build_feature(name, task, repo, accept={rel: contenido}, files=[...], contract=[...])`.
Uso como CLI (bench o feature ya configurada por el llamador): `python -m mmorch.sdlc --task <bench> --wt <dir>`.
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import time
import types
from typing import Any

from . import bench
from .paths import logs_dir
from .providers import call, register_run_tracker, unregister_run_tracker

# Globals de corrida: los fija configure() (antes venian de argv). Ninguna funcion los lee antes de configure().
TASK_NAME = ""
WT = pathlib.Path(".")
PHASE = ""
FEAT: dict | None = None
TASK: Any = None  # SimpleNamespace(name, task, accept_files) o bench.BenchTask
CONTRACT: list[str] = []
TESTS_PREFIX = "tests_accept/"
MAX_FIX = 3
LOG = pathlib.Path("run-log.json")
REVIEW_REL = "tests_accept/test_review_sdlc.py"
ACCEPT_CMD: str | None = None  # aceptacion por comando (repair / payload sin tests nombrados)
RUNS = logs_dir() / "sdlc"          # baseline de suite por sha, casos de diagnostico, copia del run-log
REASONER_TRIES = 2
WRITER = os.environ.get("SDLC_WRITER", "deepseek-reasoner")
CODER = os.environ.get("SDLC_CODER", "deepseek-v4-pro")
DIAG = os.environ.get("SDLC_DIAG", WRITER)  # medicion 2026-09-15: diagnostico con/sin razonamiento
PY = sys.executable
METRICS = logs_dir() / "metrics.jsonl"
# El pipeline es agnostico: compila/lintea/acepta por COMANDO (sdlc.toml). Esta tabla solo sugiere el compile_cmd en `init`.
_LANG_HINT = {"py": "python -m compileall -q .", "java": "mvn -q test-compile", "kt": "gradle -q compileTestKotlin",
              "ts": "npx tsc --noEmit", "tsx": "npx tsc --noEmit", "jsx": "npx tsc --noEmit", "js": "node --check {files}", "go": "go build ./... && go vet ./...",
              "rs": "cargo check --tests", "cpp": "cmake --build build", "cc": "cmake --build build", "c": "cmake --build build",
              "cs": "dotnet build --no-restore", "swift": "swift build", "rb": "ruby -c {files}", "php": "php -l {files}"}


def _exts() -> list[str]:
    """Extensiones que el pipeline puede escribir (sdlc.toml `ext`, default py)."""
    e = CFG.get("ext") or ["py"]
    return [str(x).lstrip(".") for x in e]


def _file_re() -> str:
    return r"((?:[\w.-]+/)*[\w.-]+\.(?:" + "|".join(re.escape(x) for x in _exts()) + r"))"


def _es_py(f: str) -> bool:
    return f.endswith(".py")
TPL = pathlib.Path(__file__).resolve().parent / "sdlc_templates"  # spec-kit recortado (tickets 02 y 11)


def _toml(root) -> dict:
    """`sdlc.toml` en la raiz del repo (ticket 11): accept_cmd, suite, files (techo), approve_accept, topes."""
    import tomllib
    p = pathlib.Path(root) / "sdlc.toml"
    return tomllib.loads(p.read_text(encoding="utf-8")) if p.exists() else {}


def _cfg():
    """Topes del ticket 07 + `approve_accept` (ticket 12 D5). sdlc.toml de la raiz del worktree pisa los defaults."""
    d = {"usd_max": 3.0, "stall_rounds": 2, "diff_novelty_min": 0.10, "cmd_timeout_s": 600, "suite_timeout_s": 1800,
         "approve_accept": True}
    if FEAT and FEAT.get("repo"):
        d.update(_toml(FEAT["repo"]))  # sdlc.toml recien adoptado y sin commitear: el worktree (HEAD) aun no lo tiene
    d.update(_toml(WT))
    legacy = WT / "docs" / "sdlc" / "sdlc.toml"
    if legacy.exists():
        import tomllib
        d.update(tomllib.loads(legacy.read_text(encoding="utf-8")))
    return d


def _repo_python() -> str:
    """Interprete del repo (ticket 12): `python` de sdlc.toml, o el venv sembrado en el worktree (el server siembra
    .venv/venv), o el de mmorch. Sin esto la suite de un repo con dependencias propias corre con el Python equivocado."""
    for c in ([CFG["python"]] if CFG.get("python") else []) + [
            ".venv/Scripts/python.exe", ".venv/bin/python", "venv/Scripts/python.exe", "venv/bin/python"]:
        if (WT / c).exists():
            return str(WT / c)
    return sys.executable


CFG: dict = {}
state: dict = {}
SNAP: dict[str, str] = {}
SEEN: set[str] = set()
_RUN_USD: dict = {"usd": 0.0}  # acumulador por-run de providers (W3.4): cada api-call le suma su costo


class StageFailed(Exception):
    """Una etapa o el tope de USD fallo: el llamador (CLI, server) decide; el run-log ya quedo escrito."""

    def __init__(self, stage: str, note: str):
        super().__init__(f"etapa {stage} fallo: {note}")
        self.stage, self.note = stage, note


def configure(task, *, contract, feat=None, wt=None, phase=None, max_fix=3, writer=None, coder=None, diag=None,
              accept_cmd=None):
    """Fija la corrida. `task` tiene .name, .task y .accept_files ({rel: contenido}); bench.get_task sirve tal cual.
    `feat` (dict repo/files/suite) = feature sobre un repo existente; None = task del bench (paquete nuevo)."""
    global TASK_NAME, WT, PHASE, FEAT, TASK, CONTRACT, TESTS_PREFIX, MAX_FIX, LOG, REVIEW_REL, CFG, WRITER, CODER, DIAG, ACCEPT_CMD, PY
    ACCEPT_CMD = accept_cmd
    TASK_NAME, TASK, FEAT, CONTRACT = task.name, task, feat, list(contract)
    PHASE = phase or f"sdlc-{TASK_NAME}-r1"
    # Worktrees FUERA de Desktop/Claude: codegraph indexa ese workspace y su MCP dejaba de conectar.
    WT = pathlib.Path(wt or (pathlib.Path(tempfile.gettempdir()) / "sdlc-runs" / f"{TASK_NAME}-r1"))
    TESTS_PREFIX = "tests/" if FEAT else "tests_accept/"
    MAX_FIX = int(max_fix)
    LOG = WT / "docs" / "sdlc" / "run-log.json"
    REVIEW_REL = f"{TESTS_PREFIX}test_review_sdlc.py"
    WRITER = writer or os.environ.get("SDLC_WRITER", "deepseek-reasoner")
    CODER = coder or os.environ.get("SDLC_CODER", "deepseek-v4-pro")
    DIAG = diag or os.environ.get("SDLC_DIAG", WRITER)
    CFG = _cfg()
    PY = _repo_python()
    state.clear()
    state.update({"proto": "mmorch.sdlc", "task": TASK_NAME, "stages": [], "gates": [], "calls": 0,
                  "human_interventions": 0, "claude_calls": 0, "gate_rejects": [], "escalated_to_claude": False})
    SNAP.clear(); SEEN.clear(); _orig_files.clear()
    RUNS.mkdir(parents=True, exist_ok=True)


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    try:
        r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
                 pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
    except RuntimeError as e:  # D13: el reasoner agoto 32k tokens razonando -> un intento con el coder
        if "respuesta vacia" not in str(e) or model == CODER:
            raise
        rec_gate("presupuesto-razonamiento", False, f"{model}: {str(e)[:120]}; reintento con {CODER}")
        return llm(CODER, system, user, timeout)
    usd = max(ledger_usd() or 0, round(_RUN_USD.get("usd", 0.0), 4))  # ledger (todo el proceso) o tracker (este run)
    if usd > CFG["usd_max"]:
        rec_gate("usd-tope", False, f"US${usd} > {CFG['usd_max']}")
        write_supervision(f"ESCALATE_HUMAN: tope USD {usd} superado")
        _flush()
        raise StageFailed("usd-tope", f"US${usd}: escala a humano")
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
                raise StageFailed(name, note)
        return run
    return deco


def _flush():
    LOG.parent.mkdir(parents=True, exist_ok=True)
    LOG.write_text(json.dumps(state, indent=1, ensure_ascii=False), encoding="utf-8")


def strip_fence(t):
    # D2 r1: project_integrate.py contiene "```" dentro de un string; el recorte perezoso cortaba el archivo.
    # D11-diag2: solo cuentan las vallas que ocupan una LINEA entera (```lang); una valla dentro de un string
    # va en medio de una linea. Se toma de la primera valla-linea a la ultima; sin dos vallas, el texto entero.
    fences = [m for m in re.finditer(r"(?m)^[ 	]*```\w*[ 	]*$", t)]
    if len(fences) < 2:
        return t.strip()
    return t[fences[0].end():fences[-1].start()].strip()


def one_file(code: str, rel: str) -> str:
    """Gate un-archivo (r1 2026-09-11): el coder imito las cabeceras "# path" del prompt y pego 3 archivos en uno."""
    parts = re.split(r"(?m)^(?:#|//)\s*" + _file_re() + r"\s*$", code)
    if len(parts) < 3:
        return code
    secs = {parts[i]: parts[i + 1].strip() for i in range(1, len(parts), 2)}
    rec_gate("un-archivo", rel in secs, f"{rel}: recorte de {list(secs)}")
    return secs.get(rel, code)


def sh(cmd, timeout: float | None = None, keep: int = 6000):
    """Lista = exec directo; str = comando shell (accept_cmd del repo). `python` en un comando = el interprete del repo."""
    # PYTHONDONTWRITEBYTECODE: dos mutantes del mismo tamaño escritos en el mismo segundo reusaban el .pyc del anterior
    # y el mutante "sobrevivia" (medido en Adepor 2026-09-15: 2 falsos vivos de 7).
    env = {**os.environ, "PATH": os.path.dirname(PY) + os.pathsep + os.environ.get("PATH", ""), "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        p = subprocess.run(cmd, cwd=WT, capture_output=True, text=True, encoding="utf-8", errors="replace",
                           timeout=timeout or CFG["cmd_timeout_s"], shell=isinstance(cmd, str), env=env)
    except subprocess.TimeoutExpired:
        return False, f"TIMEOUT {timeout or CFG['cmd_timeout_s']}s: {(cmd if isinstance(cmd, str) else ' '.join(cmd))[:200]}"
    return p.returncode == 0, (p.stdout + p.stderr)[-keep:]


def _cmd(key: str, files) -> str:
    """Comando de sdlc.toml; `{files}` = archivos entre comillas (compilar/lintear solo lo que toca el plan: un lint
    global sobre un repo con errores previos bloquearia toda corrida)."""
    return str(CFG[key]).replace("{files}", " ".join(f'"{f}"' for f in files))


def _suite_args(tag: str) -> list[str]:
    """D8/D10: 594 ERROR de setup = pytest no puede borrar el --basetemp de la corrida ANTERIOR porque un proceso
    hijo de algun test (codegraph.db abierto) sigue vivo y lo bloquea en Windows. Basetemp unico por corrida."""
    stamp = f"{PHASE}-{tag}-{int(time.time())}"
    return [f"{a}-{stamp}" if "pyt-sdlc" in a else a for a in (FEAT or {}).get("suite", [])]


_MUT_TEXTO = [("==", "!="), ("!=", "=="), ("<=", ">="), (">=", "<="), (" < ", " >= "), (" > ", " <= "),
              ("&&", "||"), ("||", "&&"), ("true", "false"), ("false", "true"), (" + ", " - "), (" - ", " + ")]


def _mutantes_texto(code: str, max_n: int = 8) -> list[str]:
    """Mutantes para lenguajes sin AST en stdlib (Rust, JS, Go, C...): un swap de operador/booleano por mutante,
    solo en lineas de codigo (no comentarios ni imports). Un mutante que no compila se descarta (mortinato)."""
    lines = code.split("\n")
    out: list[str] = []
    for i, line in enumerate(lines):
        st = line.strip()
        if not st or st.startswith(("//", "#", "*", "/*", "use ", "import ", "require(", "mod ", "package ")):
            continue
        for a, b in _MUT_TEXTO:
            if a in line:
                m = lines[:i] + [line.replace(a, b, 1)] + lines[i + 1:]
                out.append("\n".join(m))
                if len(out) >= max_n:
                    return out
    return out


def _accept_mata() -> bool:
    """True si la aceptacion FALLA con el mutante puesto (mutante muerto)."""
    if TASK.accept_files and all(_es_py(f) for f in TASK.accept_files):
        ok, _ = sh([PY, "-m", "pytest", *_accept_paths(), "-q", "-x", "-p", "no:cacheprovider"])
    else:
        ok, _ = sh(str(ACCEPT_CMD or CFG.get("accept_cmd") or "false"))
    return not ok


def gate_mutacion() -> tuple[bool, str]:
    """Ticket 08 D2 (opcion D): el test de aceptacion mata mutantes del codigo final. Score = muertos / total sobre los
    archivos del plan (mutantes de checkers._mutants: operadores, comparaciones, booleanos, enteros). Bloquea SOLO con
    `mutation_min` en sdlc.toml; sin numero medido, observa y deja state["mutation_score"]."""
    from .checkers import _mutants
    if not TASK.accept_files and not (ACCEPT_CMD or CFG.get("accept_cmd")):
        return rec_gate("mutacion", True, "sin oraculo de aceptacion: sin mutacion")
    killed = total = 0
    compile_cmd = CFG.get("compile_cmd")
    for f in state.get("plan_files", []):
        p = WT / f
        if not p.exists():
            continue
        orig = p.read_text(encoding="utf-8")
        for m in (_mutants(orig, max_n=8) if _es_py(f) else _mutantes_texto(orig, max_n=8)):
            p.write_text(m, encoding="utf-8")
            try:
                if not _es_py(f) and compile_cmd and not sh(_cmd("compile_cmd", [f]))[0]:
                    continue  # mortinato: no compila, no cuenta
                total += 1
                if _accept_mata():
                    killed += 1
            finally:
                p.write_text(orig, encoding="utf-8")
    if not total:
        state["mutation_score"] = None
        return rec_gate("mutacion", True, "sin mutantes posibles")
    score = round(killed / total, 3)
    state["mutation_score"] = score
    minimo = CFG.get("mutation_min")
    if minimo is not None and score < float(minimo):
        return rec_gate("mutacion", False, f"mutation score {score} < {minimo} ({killed}/{total})")
    return rec_gate("mutacion", True, f"mutation score {score} ({killed}/{total})" + ("" if minimo is not None else " (observa: sin mutation_min)"))


def _en_base(fn):
    """Corre fn() con los archivos del plan en su version de HEAD (baseline) y despues los restaura."""
    keep = {f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()}
    # 2026-09-16: un archivo NUEVO del plan no existe en HEAD; `git checkout HEAD -- nuevo` explotaba y el job del server
    # terminaba en error sin detalle (macro-leadlag, export-mastery). En la base, el archivo nuevo simplemente no esta.
    en_head = [f for f in keep if subprocess.run(["git", "cat-file", "-e", f"HEAD:{f}"], cwd=WT, capture_output=True).returncode == 0]
    if en_head:
        subprocess.run(["git", "checkout", "HEAD", "--", *en_head], cwd=WT, check=True)  # sin stash: la pila es compartida
    for f in keep:
        if f not in en_head:
            (WT / f).unlink()
    try:
        return fn()
    finally:
        for f, c in keep.items():
            _write(f, c)


def _suite_cmd_total(sha: str) -> tuple[bool, str]:
    """Otros lenguajes (`suite_cmd` en sdlc.toml): sin nombres de tests, compara verde/rojo contra el baseline."""
    # ponytail: exit code, no nombres; parsear la salida del runner (vitest, surefire) cuando un baseline rojo lo pida
    base_file = RUNS / f"suite-baseline-{sha}-cmd.txt"
    if not base_file.exists():
        bok, blog = _en_base(lambda: sh(str(CFG["suite_cmd"]), timeout=CFG["suite_timeout_s"], keep=400000))
        _write("docs/sdlc/suite-baseline.log", blog)
        if "TIMEOUT" in blog:
            return rec_gate("suite-total", False, f"baseline: {blog[:200]}")
        base_file.write_text("verde" if bok else "rojo", encoding="utf-8")
    ok, log = sh(str(CFG["suite_cmd"]), timeout=CFG["suite_timeout_s"], keep=400000)
    _write("docs/sdlc/suite.log", log)
    base_verde = base_file.read_text(encoding="utf-8") == "verde"
    state["suite_total"] = {"suite_cmd": ok, "baseline_verde": base_verde}
    if not base_verde:
        return rec_gate("suite-total", True, "baseline rojo: regresion no medible por exit code (observa)")
    return rec_gate("suite-total", ok, "sin fallos nuevos" if ok else f"baseline verde y ahora rojo: {log[-300:]}")


def gate_suite_total() -> tuple[bool, str]:
    """Regresion total (ticket 13): la suite entera del repo no gana fallos nuevos respecto del baseline.
    mmorch tarda ~20 min y trae 4 rojos previos (medido 2026-09-11): se compara por NOMBRE, no por verde."""
    # D3 r1b: el baseline medido en el arbol principal no vale (tests sin commitear, metadata del paquete).
    # Se mide EN el worktree, sobre el commit base, una vez por sha; se cachea en suite-baseline-<sha>.txt.
    sha = subprocess.run(["git", "rev-parse", "--short", state.get("base_sha", "HEAD~1")], cwd=WT, capture_output=True, text=True).stdout.strip()
    if CFG.get("suite_cmd"):
        return _suite_cmd_total(sha)
    base_file = RUNS / f"suite-baseline-{sha}.txt"
    if not base_file.exists():
        _, blog = _en_base(lambda: sh([PY, "-m", "pytest", *_suite_args("base")], timeout=CFG["suite_timeout_s"], keep=400000))
        _write("docs/sdlc/suite-baseline.log", blog)
        if "TIMEOUT" in blog:  # D15: un baseline vacio por timeout se cacheaba y volvia "nuevos" a los rojos previos
            return rec_gate("suite-total", False, f"baseline: {blog[:200]}")
        base_file.write_text("\n".join(re.findall(r"(?m)^(?:FAILED|ERROR) \S+", blog)) + "\n", encoding="utf-8")
    # D8: la suite del worktree traia 594 ERROR (setup) que el gate no miraba porque solo leia FAILED.
    base = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", base_file.read_text(encoding="utf-8")))
    ok, log = sh([PY, "-m", "pytest", *_suite_args("final")], timeout=CFG["suite_timeout_s"], keep=400000)
    _write("docs/sdlc/suite.log", log)
    now = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", log))
    new = sorted(now - base)
    if new and "TIMEOUT" not in log:
        # D2 r1: 3 tests de providers fallaron en la suite y pasaron solos -> flaky bajo carga.
        # Un fallo nuevo cuenta como regresion solo si se repite AISLADO (determinista, sin juicio).
        _, rlog = sh([PY, "-m", "pytest", *new[:50], "-q", "-rfE", "-p", "no:cacheprovider"], keep=400000)
        flaky = sorted(set(new) - set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", rlog)))
        new = sorted(set(new) - set(flaky))
        state["suite_flaky"] = flaky
    state["suite_total"] = test_counts(log) | {"new_failures": new, "baseline_failures": len(base)} if test_counts(log) else {"log": log[-300:]}
    if "TIMEOUT" in log:
        return rec_gate("suite-total", False, log[:200])
    return rec_gate("suite-total", not new, "sin fallos nuevos" if not new else f"fallos nuevos: {new}")


def _accept_paths():
    paths = list(TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        paths.append(REVIEW_REL)
    return paths


def accept():
    cmd = ACCEPT_CMD or CFG.get("accept_cmd")  # payload o sdlc.toml
    if cmd and (not TASK.accept_files or not all(_es_py(f) for f in TASK.accept_files)):
        return sh(str(cmd))  # sin tests nombrados, o tests en otro lenguaje: el comando del repo es el oraculo
    return sh([PY, "-m", "pytest", *_accept_paths(), "-q", "-p", "no:cacheprovider"])


def test_counts(log):
    # D3: con -rf el log repite "N failed" dentro de los asserts; se cuenta SOLO la linea de resumen final
    # D4/D5: los "N failed" dentro de los asserts de -rf inflaban el conteo (594): solo la ULTIMA linea de pytest
    lines = [l for l in log.strip().splitlines() if l.strip()]
    tail = [l for l in lines[-3:] if re.search(r"\d+ (?:passed|failed|errors?)\b", l)]
    if tail:
        log = tail[-1]
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


def _need_files() -> list[str]:
    """Archivos que el plan DEBE incluir. Feature de repo: lista explicita (D4: la tarea nombra archivos que NO se
    tocan, el regex los tomaba como obligatorios). Bench: los paths que nombra el enunciado."""
    if FEAT:
        return list(FEAT["files"])
    return list(dict.fromkeys(re.findall(r"[\w/.-]+\.(?:" + "|".join(_exts()) + ")", TASK.task)))


def _techo() -> list[str]:
    """Patrones (fnmatch) de lo que el plan PUEDE escribir; vacio = solo los obligatorios (bench)."""
    return list((FEAT or {}).get("techo") or [])


def _calza(f: str, pat: str) -> bool:
    """fnmatch con `**/` opcional: `pkg/**/*.py` incluye `pkg/a.py` (fnmatch solo exigia un directorio intermedio)."""
    import fnmatch
    return fnmatch.fnmatch(f, pat) or fnmatch.fnmatch(f, pat.replace("**/", ""))


def _en_techo(f: str) -> bool:
    t = _techo()
    return f in _need_files() or not t or any(_calza(f, pat) for pat in t)  # sin techo (bench, FEATURES viejas): todo vale


def gate_plan_allowlist(plan_md: str, files: list[str]) -> tuple[bool, str]:
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    block = m.group(1) if m else plan_md
    # un cableo puede listar tests/test_capas.py (esta en FEAT["files"]); cualquier otro tests/ sigue prohibido
    ajenos = [t for t in re.findall(r"(?im)^\s*[-*]+\s*`?(" + re.escape(TESTS_PREFIX) + r"[\w/.-]*)", block) if t not in _need_files()]
    if ajenos:
        return rec_gate("plan-allowlist", False, f"plan lista {ajenos} como archivo a escribir")
    need = [f for f in _need_files() if f not in files]
    if need:
        return rec_gate("plan-allowlist", False, f"plan omite {need}")
    fuera = [f for f in files if not _en_techo(f)]
    if fuera:
        return rec_gate("plan-allowlist", False, f"plan lista archivos fuera del techo de sdlc.toml: {fuera}")
    return rec_gate("plan-allowlist", True, f"{files}")


def _test_names() -> set[str]:
    """Nombres test_... de la aceptacion: `def test_` en Python; en otro lenguaje, funcion o titulo (`it('test_R1_...')`)."""
    return {n for rel in TASK.accept_files
            for n in re.findall(r"(?m)^def (test_\w+)" if _es_py(rel) else r"\b(test_\w+)", (WT / rel).read_text(encoding="utf-8"))}


def gate_traza_spec(spec_md: str, tests: set[str]) -> tuple[bool, str]:
    """Trazabilidad lado spec (ticket 02): IDs R<n> y tabla que cita tests existentes por nombre."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    if not ids:
        return rec_gate("trazabilidad-spec", False, "sin IDs R<n>")
    if not tests and not getattr(TASK, "accept_files", None):
        return rec_gate("trazabilidad-spec", True, f"{len(ids)} IDs; sin tests nombrados (accept_cmd es el oraculo)")
    m = re.search(r"## Trazabilidad\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    rows = re.findall(r"(?m)^\|\s*(R\d+)\s*\|([^|]*)\|", m.group(1) if m else "")
    cov = {rid: set(re.findall(r"test_\w+", cell)) for rid, cell in rows}
    sin_test = sorted(i for i in ids if not cov.get(i))
    inexist = sorted(t for ts in cov.values() for t in ts if t not in tests)
    if sin_test or inexist:
        return rec_gate("trazabilidad-spec", False, f"sin test: {sin_test}; tests inexistentes: {inexist}")
    return rec_gate("trazabilidad-spec", True, f"{len(ids)} IDs cubiertos")


def gate_traza_plan(spec_md: str, plan_block: str, files: list[str]) -> tuple[bool, str]:
    """Trazabilidad lado plan: cada R<n> de la spec aparece en el plan; cada archivo cita >= 1 R<n>."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    huerfanos = sorted(i for i in ids if not re.search(rf"\b{i}\b", plan_block))
    items = re.findall(r"(?m)^\s*[-*]\s*(.*)$", plan_block)
    sin_id = [f for f in files if not any(f in it and re.search(r"\bR\d+\b", it) for it in items)]
    if huerfanos or sin_id:
        return rec_gate("trazabilidad-plan", False, f"IDs sin unidad: {huerfanos}; archivos sin ID: {sin_id}")
    return rec_gate("trazabilidad-plan", True, "ok")


def gate_clarificaciones(spec_md: str) -> tuple[bool, str]:
    """Salida de la revision de spec (ticket 11, de clarify.md): seccion con 0..5 preguntas respondidas."""
    m = re.search(r"## Clarificaciones\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    if not m:
        return rec_gate("spec-review", False, "falta ## Clarificaciones")
    qs = re.findall(r"(?m)^- P\d+: .+\| R: \S.*\| afecta: \S+", m.group(1))  # D11-diag: "afecta: Contrato" tambien vale
    if not qs and re.search(r"sin preguntas|0 preguntas|ninguna pregunta|no hay preguntas", m.group(1), re.I):
        return rec_gate("spec-review", True, "0 preguntas")  # D11-diag: Claude escribio "0 preguntas" en vez del literal
    if not 1 <= len(qs) <= 5:
        return rec_gate("spec-review", False, f"{len(qs)} preguntas validas (esperado 1..5)")
    return rec_gate("spec-review", True, f"{len(qs)} preguntas respondidas")


def _tree() -> dict[str, str]:
    """path -> sha de todo archivo del worktree (tracked + nuevos), para medir que toco una pasada de Claude."""
    out = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], cwd=WT, capture_output=True, text=True).stdout
    return {p: sha_file(p) for p in out.splitlines() if p and (WT / p).is_file()}


def gate_alcance(gid: str, before: dict[str, str], allowed: tuple[str, ...]) -> tuple[bool, str]:
    """Una pasada de Claude solo puede tocar `allowed` (r4b: la revision de spec implemento core.py y multi.py).
    Lo demas vuelve al estado previo: tracked/staged con checkout, nuevo con clean. Determinista, USD 0."""
    after = _tree()
    changed = [p for p in set(before) | set(after) if before.get(p) != after.get(p)]
    fuera = sorted(p for p in changed if not p.startswith(allowed) and not p.startswith("docs/sdlc/supervision"))
    if fuera:
        subprocess.run(["git", "checkout", "--", *[p for p in fuera if p in before]], cwd=WT, capture_output=True)
        subprocess.run(["git", "clean", "-fq", "--", *[p for p in fuera if p not in before]], cwd=WT, capture_output=True)
    return rec_gate(gid, not fuera, "ok" if not fuera else f"toco fuera de alcance, revertido: {fuera}")


def gate_baseline() -> tuple[bool, str]:
    bad = [rel for rel, h in SNAP.items() if not (WT / rel).exists() or sha_file(rel) != h]
    return rec_gate("baseline-intacto", not bad, "ok" if not bad else f"toco {bad}")


def gate_compile(files) -> tuple[bool, str]:
    """G3: .py -> py_compile; otros lenguajes -> `compile_cmd` de sdlc.toml (sin el, se anota y no bloquea)."""
    pys = [f for f in files if _es_py(f)]
    if pys:
        ok, log = sh([PY, "-m", "py_compile", *pys])
        if not ok:
            return rec_gate("G3-compile", False, log[-800:])
    otros = [f for f in files if not _es_py(f)]
    if otros:
        cmd = CFG.get("compile_cmd")
        if not cmd:
            return rec_gate("G3-compile", True, f"sin compile_cmd para {otros}: no se compila")
        ok, log = sh(_cmd("compile_cmd", otros))
        return rec_gate("G3-compile", ok, "compila" if ok else log[-800:])
    return rec_gate("G3-compile", True, "compila")


def gate_test_compile() -> tuple[bool, str]:
    """Equivalente a mvn test-compile: los tests importan y se recolectan."""
    if not _accept_paths():
        return rec_gate("test-compile", True, "sin tests nombrados (accept_cmd)")
    if not all(_es_py(f) for f in _accept_paths()):
        cmd = CFG.get("compile_cmd")
        if not cmd:
            return rec_gate("test-compile", True, "tests no-Python sin compile_cmd: no se compila")
        ok, log = sh(_cmd("compile_cmd", _accept_paths()))
        return rec_gate("test-compile", ok, "ok" if ok else log[-800:])
    ok, log = sh([PY, "-m", "pytest", *_accept_paths(), "--collect-only", "-q", "-p", "no:cacheprovider"])
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
    if code and not code.endswith("\n"):
        code += "\n"  # D4: strip_fence borraba la linea final y el diff mostraba "No newline at end of file"
    p.write_text(code, encoding="utf-8")


def _docstrings(files) -> dict:
    import ast
    out: dict[str, str | None] = {}
    for f in files:
        if not (WT / f).exists():
            out[f] = None
            continue
        text = (WT / f).read_text(encoding="utf-8")
        if _es_py(f):
            try:
                out[f] = ast.get_docstring(ast.parse(text))
            except SyntaxError:
                out[f] = None
        else:  # otros lenguajes: la cabecera de comentario del archivo (//! //, #, /* */) es el "docstring de modulo"
            m = re.match(r"(?:\s*(?://[^\n]*|#[^\n]*|/\*[\s\S]*?\*/)\n)+", text)
            out[f] = m.group(0).strip() if m else None
    return out


def gate_clones(written: dict) -> tuple[bool, str]:
    """D6: el coder escribio el contenido de plugins.py dentro de plugin_worker.py. Dos archivos del plan
    con > 80% de lineas identicas es un clon, no una feature. Determinista, USD 0."""
    files = list(written)
    for i, a in enumerate(files):
        la = {l.strip() for l in written[a].splitlines() if l.strip()}
        for b in files[i + 1:]:
            lb = {l.strip() for l in written[b].splitlines() if l.strip()}
            if la and lb and len(la & lb) / min(len(la), len(lb)) > 0.8:
                return rec_gate("sin-clones", False, f"{a} y {b} comparten > 80% de lineas")
    return rec_gate("sin-clones", True, "ok")


_orig_files: dict[str, str] = {}


def gate_cambio_minimo(before: dict, written: dict) -> tuple[bool, str]:
    """D11: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda. Sobre un archivo que ya existia,
    borrar mas del 30% de sus lineas (minimo 40) sin que la tarea diga 'reescrib' es un cambio no pedido. USD 0."""
    if "reescrib" in TASK.task.lower():
        return rec_gate("cambio-minimo", True, "la tarea pide reescribir")
    malos = []
    for f, orig in _orig_files.items():
        if f not in written or not orig.strip():
            continue
        old_lines = [l for l in orig.splitlines() if l.strip()]
        new_set = {l.strip() for l in written[f].splitlines()}
        borradas = sum(1 for l in old_lines if l.strip() not in new_set)
        if borradas > max(40, int(0.3 * len(old_lines))):
            malos.append(f"{f}: {borradas} de {len(old_lines)} lineas originales desaparecieron")
    return rec_gate("cambio-minimo", not malos, "ok" if not malos else "; ".join(malos))


def gate_docstring(before: dict) -> tuple[bool, str]:
    """D4: el coder borro el docstring del modulo (22 lineas) y ningun test lo ve. Determinista, USD 0:
    un docstring de modulo que existia antes tiene que seguir existiendo, salvo que la tarea hable de docstrings."""
    if "docstring" in TASK.task.lower():
        return rec_gate("docstring-intacto", True, "la tarea lo cubre")
    after = _docstrings(before)
    perdidos = [f for f, d in before.items() if d and not after.get(f)]
    return rec_gate("docstring-intacto", not perdidos, "ok" if not perdidos else f"docstring de modulo borrado en {perdidos}")


def _tests_text():
    t = "\n\n".join(f"# {k}\n{(WT / k).read_text(encoding='utf-8')}" for k in TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        t += "\n\n# test_review_sdlc.py (revision de Claude, no se toca)\n" + (WT / REVIEW_REL).read_text(encoding="utf-8")
    return t


def write_supervision(note: str):
    p = WT / "docs" / "sdlc" / "supervision.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    prev = p.read_text(encoding="utf-8") if p.exists() else "# supervision.md — driver_py\n\n"
    p.write_text(prev + f"\n## candidato {time.strftime('%H:%M:%S')}\n{note}\n", encoding="utf-8")


def _revert(backup):
    for f, old in backup.items():
        _write(f, old)


def _dump_diag_case(log: str) -> None:
    d = RUNS / "diag-cases"; d.mkdir(parents=True, exist_ok=True)
    case = {"task": TASK_NAME, "phase": PHASE, "base_sha": state.get("base_sha"), "log": log[-20000:],
            "files": _all_code(), "orig": dict(_orig_files), "tests": _tests_text(), "task_text": TASK.task}
    (d / f"{PHASE}-{int(time.time())}.json").write_text(json.dumps(case, ensure_ascii=False), encoding="utf-8")


def reasoner_rounds(log) -> tuple[bool, str]:
    """Nivel 2: dos intentos con deepseek-reasoner. Una instruccion por archivo."""
    # bench de diagnostico (2026-09-15): cada entrada a la escalera queda como caso replayable (log + archivos + tests),
    # para medir diagnosticadores con/sin razonamiento SIN depender de que una corrida nueva falle justo ahi.
    _dump_diag_case(log)
    for i in range(REASONER_TRIES):
        written = _all_code()
        pick = llm(DIAG, 'Sos un diagnosticador. Respondes SOLO JSON: {"files": [paths], "instructions": {path: "una instruccion"}}. Una instruccion por archivo.',
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
                      f"Aplica ESTA instruccion a {f}. No toques los tests.\n"
                      f"INSTRUCCION: {instr.get(f, 'corregi el fallo del test')}\n\n"
                      f"SALIDA:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
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
    before = _tree()
    r = run_claude(f"Los tests de aceptacion fallan. Arregla el codigo en {state['plan_files']} hasta que "
                   f"`pytest {' '.join(_accept_paths())} -q` pase. NO toques tests ni docs. Al final responde en una linea que cambiaste.\n\n"
                   f"TAREA:\n{TASK.task}\n\nSALIDA:\n{log}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"nivel 3 Claude (rc={r.get('returncode')}): {(r.get('result') or '')[:2000]}")
    gate_alcance("claude-fix-alcance", before, tuple(state["plan_files"]))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    ok, log = accept()
    if ok:
        state["suite_total"] = test_counts(log)
    return ok, "Claude verde" if ok else "Claude rojo: escala a humano"


def self_check() -> int:
    """Cero API: allowlist, novedad, conteo pytest."""
    fails = []
    need = _need_files()
    good = "## Archivos\n" + "".join(f"- `{f}`\n" for f in need) + "\n## Prueba\npytest\n"
    if not gate_plan_allowlist(good, need)[0]:
        fails.append("allowlist-good")
    if gate_plan_allowlist(f"## Archivos\n- {TESTS_PREFIX}test_x.py\n- {need[0]}", need)[0]:
        fails.append("allowlist-tests")
    if gate_plan_allowlist(good, need[:-1])[0]:
        fails.append("allowlist-omite")
    SEEN.clear()
    if _novelty({"A": "x\ny"}, {"A": "x\nz"}) != 1.0 or _novelty({"A": "x\nz"}, {"A": "x\ny"}) != 0.0:
        fails.append("novelty")
    if test_counts("2 failed, 1 passed in 0.1s") != {"run": 3, "failed": 2} or test_counts("3 passed in 0.1s") != {"run": 3, "failed": 0}:
        fails.append("counts")
    if one_file("# limiter/__init__.py\nfrom x import y\n\n# limiter/core.py\nclass TokenBucket: pass\n", "limiter/core.py") != "class TokenBucket: pass" \
            or one_file("class A: pass", "a.py") != "class A: pass":
        fails.append("un-archivo")
    tests = {"test_a", "test_b"}
    sp = "## Requisitos\n- R1: x\n- R2: y\n\n## Trazabilidad\n| ID | tests |\n|---|---|\n| R1 | test_a |\n| R2 | test_a, test_b |\n"
    if not gate_traza_spec(sp, tests)[0]:
        fails.append("traza-spec-good")
    if gate_traza_spec(sp.replace("| R2 | test_a, test_b |\n", ""), tests)[0] or gate_traza_spec(sp.replace("test_b", "test_zz"), tests)[0]:
        fails.append("traza-spec-bad")
    pb = "\n- `limiter/core.py` [R1] [P]\n- `limiter/multi.py` [R2]\n"
    if not gate_traza_plan(sp, pb, ["limiter/core.py", "limiter/multi.py"])[0]:
        fails.append("traza-plan-good")
    if gate_traza_plan(sp, pb.replace("[R2]", ""), ["limiter/core.py", "limiter/multi.py"])[0]:
        fails.append("traza-plan-bad")
    cl = "## Clarificaciones\n\n- P1: cap? | R: SUPUESTO: float | afecta: R1\n"
    if not gate_clarificaciones(cl)[0] or not gate_clarificaciones("## Clarificaciones\n- sin preguntas: la spec cubre el barrido.\n")[0] \
            or gate_clarificaciones("## Casos\n")[0] or gate_clarificaciones("## Clarificaciones\n- P1: cap? | R:  | afecta: R1\n")[0]:
        fails.append("clarificaciones")
    print("self-check", "FAIL" if fails else "PASS", fails)
    return 1 if fails else 0


@stage("1-aceptacion")
def aceptacion():
    """Etapa 1 (ticket 12 D5): Claude escribe el test de aceptacion desde la tarea. Gates deterministas: nombra
    test_R<n>_..., compila (collect-only) y FALLA en HEAD (rojo por diseño). Con `approve_accept` (default) la corrida
    se detiene en `awaiting_approval`: un humano (o el agente del ticket 14) aprueba y reanuda desde la etapa 2."""
    if TASK.accept_files:
        return True, "tests de aceptacion provistos por el llamador"
    from .claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    slug = re.sub(r'[^a-z0-9]+', '_', TASK_NAME.lower()).strip('_')[:40]
    ext = _exts()[0]
    rel = str(CFG.get("accept_test", f"{TESTS_PREFIX}test_sdlc_{{slug}}.{ext}")).replace("{slug}", slug)
    state["claude_calls"] += 1
    before = _tree()
    marco = "pytest" if _es_py(rel) else f"framework de tests del repo; lo corre `{ACCEPT_CMD or CFG.get('accept_cmd')}`"
    r = run_claude(f"Escribi SOLO el archivo {rel}: el test de aceptacion ({marco}, sin red) de esta TAREA. Un test por "
                   f"requisito, nombrados test_R1_..., test_R2_... con el ID en el nombre (funcion o titulo del test); "
                   f"comentario o docstring de modulo con el contrato "
                   f"R<n>. El test DEBE fallar hoy (el codigo aun no existe o no cumple) y pasar cuando la tarea este hecha. "
                   f"No toques ningun otro archivo.\n\nTAREA:\n{TASK.task}",
                   cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"aceptacion (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1000]}")
    if not gate_alcance("aceptacion-alcance", before, (rel,))[0] or not (WT / rel).exists():
        return False, f"Claude no dejo {rel} (o toco otros archivos)"
    text = (WT / rel).read_text(encoding="utf-8")
    ids = sorted(set(re.findall(r"(?m)^def test_(R\d+)" if _es_py(rel) else r"\btest_(R\d+)", text)))
    if not ids:
        return rec_gate("aceptacion-ids", False, "sin tests test_R<n>_...")
    if _es_py(rel):
        ok_c, log = sh([PY, "-m", "pytest", rel, "--collect-only", "-q", "-p", "no:cacheprovider"])
        if not ok_c:
            return rec_gate("aceptacion-compila", False, log[-600:])
        ok_r, _ = sh([PY, "-m", "pytest", rel, "-q", "-p", "no:cacheprovider"])
    else:  # otro lenguaje: el comando del repo compila y corre; debe estar ROJO
        ok_r, _ = sh(str(ACCEPT_CMD or CFG.get("accept_cmd") or "false"))
    if ok_r:
        return rec_gate("aceptacion-roja", False, f"{rel} ya pasa en HEAD: no mide la tarea")
    rec_gate("aceptacion-roja", True, f"{rel} roja en HEAD, R: {ids}")
    TASK.accept_files[rel] = text
    _seed_accept()
    snapshot_baseline()
    if CFG.get("approve_accept", True):
        state["awaiting_approval"] = rel
        write_supervision(f"ESPERA_HUMANO: aprobar {rel} y reanudar desde la etapa 2 (from_stage=2)")
        return False, f"esperando aprobacion humana de {rel} (reanudar desde la etapa 2)"
    return True, f"{rel}: R {ids}, roja en HEAD, sin aprobacion (approve_accept=false)"


@stage("2-spec")
def spec():
    tpl = (TPL / "spec-template.md").read_text(encoding="utf-8")
    tests = _test_names()
    ask = (f"Escribi spec.md siguiendo EXACTAMENTE esta plantilla (mismas secciones, IDs R<n> unicos, tabla de trazabilidad "
           f"que cita tests por su nombre exacto de entre {sorted(tests)}). Python 3.12, sin dependencias.\n\n"
           f"PLANTILLA:\n{tpl}\n\nTAREA:\n{TASK.task}\n\nTESTS DE ACEPTACION (no se modifican):\n{_tests_text()}\n\n"
           "Omiti las lineas de instruccion de la plantilla. Sin marcadores pendientes. Solo markdown.")
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.", ask)
    _write("docs/sdlc/spec.md", out)

    def _falla():
        missing = [s for s in CONTRACT if s not in out]
        tok, tnote = gate_traza_spec(out, tests)
        return (f"faltan tokens {missing}; " if missing else "") + ("TBD; " if "TBD" in out else "") + ("" if tok else tnote)

    why = _falla()
    if why:
        out = llm(WRITER, "Sos un ingeniero. Reescribís la spec completa. Los tokens y la tabla pedidos deben aparecer VERBATIM.",
                  f"Esta spec fallo el gate: {why}. Corregilo sin borrar el resto. Plantilla:\n{tpl}\n\nSPEC ACTUAL:\n{out}")
        _write("docs/sdlc/spec.md", out)
        why = _falla()
    if why:
        rec_gate("G1-spec-contrato", False, why)
        return False, f"G1 rechaza: {why}"
    rec_gate("G1-spec-contrato", True, "contrato cubierto, sin TBD, IDs trazados")
    return True, "G1 + trazabilidad-spec ok"


@stage("2b-spec-review")
def spec_review():
    """Claude revisa la spec con spec-review.md (de clarify.md): <= 5 preguntas, respondidas en la spec."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    guide = (TPL / "spec-review.md").read_text(encoding="utf-8")
    state["claude_calls"] += 1
    before = _tree()
    r = run_claude(f"{guide}\n\nTAREA:\n{TASK.task}\n\nEdita SOLO docs/sdlc/spec.md segun estas reglas. NO escribas codigo ni crees "
                   "ni toques ningun otro archivo: la implementacion la hace otra etapa. Responde en una linea cuantas preguntas hiciste.",
                   cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"revision de spec (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1500]}")
    gate_alcance("spec-review-alcance", before, ("docs/sdlc/spec.md",))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    ok, note = gate_clarificaciones(spec_md)
    if ok:
        ok, note = gate_traza_spec(spec_md, _test_names())  # la revision no puede romper la trazabilidad
    if not ok:  # D13-diag: Claude agrego un R5 sin test; una vuelta para que lo quite o lo funda en un R con test
        state["claude_calls"] += 1
        before = _tree()
        run_claude(f"Tu revision de docs/sdlc/spec.md fallo este gate: {note}. Cada R<n> de la spec debe tener un test "
                   f"con su ID en el nombre; NO agregues R nuevos: quitalos o fundilos en un R existente. Edita SOLO docs/sdlc/spec.md.",
                   cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
        gate_alcance("spec-review-alcance", before, ("docs/sdlc/spec.md",))
        spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
        ok, note = gate_clarificaciones(spec_md)
        if ok:
            ok, note = gate_traza_spec(spec_md, _test_names())
    return ok, note


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    ask = (f"Escribi plan.md. Formato OBLIGATORIO: seccion '## Archivos' con un item por archivo a escribir, asi:\n"
           f"- `path.py` [R1, R3] [P]\n"
           f"Cada item cita los IDs R<n> de la spec que cubre. Todo R<n> de la spec aparece en algun item. "
           f"[P] marca archivos que NO importan a otros del plan (paralelizables); los demas van en orden de dependencia. "
           f"'## Prueba' = `python -m pytest {' '.join(_accept_paths())} -q`.\n"
           f"Archivos OBLIGATORIOS en '## Archivos': {_need_files()}.\n\nSPEC:\n{spec_md}")  # D14-diag: el plan omitia test_capas
    out = llm(WRITER, "Sos un tech lead. Escribis planes ejecutables. NO regeneres los tests.", ask)
    for intento in range(2):  # D2 r1: una reescritura con el motivo del gate, como en spec
        _write("docs/sdlc/plan.md", out)
        m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", out, re.S | re.I)
        block = m.group(1) if m else out
        files = _plan_files(block)
        state["plan_files"] = files
        state["plan_parallel"] = [f for f in files if re.search(rf"{re.escape(f)}`?[^\n]*\[P\]", block)]  # ticket 05 lo ejecuta en paralelo
        ok, note = gate_plan_allowlist(out, files)
        if ok:
            ok, note = gate_traza_plan(spec_md, block, files)
        if ok or intento:
            return ok, note
        out = llm(WRITER, "Sos un tech lead. Reescribis el plan completo con el formato obligatorio.",
                  f"Este plan fallo el gate: {note}. Corregilo.\n\nPLAN ACTUAL:\n{out}\n\nSPEC:\n{spec_md}")
    return ok, note


def _plan_files(block: str) -> list[str]:
    """Archivos del plan = SOLO los items de la lista (D2 r1: una mencion en prosa 'sin tocar X.py' se colaba)."""
    items = re.findall(r"(?m)^\s*[-*]\s*`?" + _file_re() + r"`?", block)
    return [f for f in dict.fromkeys(items) if not f.startswith(TESTS_PREFIX) or f in _need_files()]  # D14-diag: test_capas es obligatorio en cableos


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    written = {}
    docs_before = _docstrings(state["plan_files"])  # gate docstring-intacto (D4)
    _orig_files.clear()
    _orig_files.update({f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()})
    for f in state["plan_files"]:
        cur = (WT / f).read_text(encoding="utf-8") if (WT / f).exists() else ""
        cur_ctx = f"ARCHIVO ACTUAL {f} (devolvelo COMPLETO con el cambio minimo):\n{cur}\n\n" if cur.strip() else ""
        out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi {f}. Tiene que importar con los archivos ya escritos y pasar los tests de aceptacion.\n\n{cur_ctx}"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nTAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\n"
                  f"ARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = one_file(strip_fence(out), f)
        _write(f, code)
        written[f] = code
        bok, bnote = gate_baseline()
        if not bok:
            return False, bnote
    def _compiles():
        ok, log = gate_compile(state["plan_files"])
        return (ok, log) if not ok else gate_test_compile()

    ok, log = _compiles()
    for _ in range(3):  # r1 2026-09-11: test-compile tambien entra al fix loop, antes cortaba sin vuelta
        if ok:
            break
        for f in state["plan_files"]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo corregido del archivo pedido, un solo archivo.",
                      f"py_compile o la importacion de los tests fallo. Corregi {f}.\n\nERROR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
            if code and code != written[f]:
                _write(f, code); written[f] = code
        ok, log = _compiles()
        if not gate_baseline()[0]:
            return False, "baseline roto en compile-fix"
    if not ok:
        return False, f"G3 rechaza tras 3 vueltas: {log[-300:]}"
    cok, cnote = gate_clones(written)
    if not cok:
        return False, cnote
    mok, mnote = gate_cambio_minimo(docs_before, written)
    if not mok:  # D11: el coder borro 216 lineas de project_loop.py para agregar 10; una vuelta de re-pedido
        for f in [f for f in written if f in mnote]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Tu version de {f} borro codigo existente que la tarea NO pide tocar ({mnote}). "
                      f"Parti del ARCHIVO ORIGINAL y aplica SOLO el cambio pedido, conservando todo lo demas.\n\n"
                      f"TAREA:\n{TASK.task}\n\nARCHIVO ORIGINAL:\n{_orig_files.get(f, '')}\n\nTU VERSION:\n{written[f]}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        mok, mnote = gate_cambio_minimo(docs_before, written)
        if not mok or not _compiles()[0]:
            return False, f"cambio-minimo: {mnote}"
    dok, dnote = gate_docstring(docs_before)
    if not dok:  # una vuelta del coder para restaurarlo; si insiste, la etapa falla
        for f in [f for f, d in docs_before.items() if d and not _docstrings([f]).get(f)]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Restaura el docstring de modulo original de {f} al inicio del archivo, sin cambiar nada mas.\n\n"
                      f"DOCSTRING ORIGINAL:\n\"\"\"{docs_before[f]}\"\"\"\n\nARCHIVO ACTUAL:\n{(WT / f).read_text(encoding='utf-8')}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        dok, dnote = gate_docstring(docs_before)
        if not dok or not _compiles()[0]:
            return False, f"docstring: {dnote}"
    return True, "G3 + test-compile + docstring ok"


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
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero. No toques los tests.",
                      f"Arregla {f}. Los tests NO se modifican.\n\nSALIDA:\n{log}\n\nTAREA:\n{TASK.task}\n\n"
                      f"TESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
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
    ladder_note = ""
    if not ok:  # escalera: reasoner x2 -> Claude. D11: antes estos caminos saltaban lint y suite total.
        ok, ladder_note = reasoner_rounds(log)
        if not ok:
            ok, ladder_note = claude_fix(log)
        if not ok:
            rec_gate("G4-aceptacion", False, ladder_note)
            write_supervision("ESCALATE_HUMAN: fix 3 + reasoner 2 + Claude agotados.")
            return False, ladder_note
    if ok and FEAT:
        lok, lnote = gate_lint()
        if not lok:  # D2 r1: el pre-commit del repo rechazo 2 errores de mypy; ahora es gate con escalera
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"ruff/mypy reportan errores nuevos. Corregi {state['plan_files'][0]} sin cambiar comportamiento.\n\n"
                      f"ERRORES:\n{lnote}\n\nARCHIVOS:\n{_joined(_all_code())}")
            code = one_file(strip_fence(out), state["plan_files"][0])
            if code:
                _write(state["plan_files"][0], code)
            lok, lnote = gate_lint()
            aok, _ = accept()
            if not (lok and aok):
                cok, cnote = claude_fix(f"ruff/mypy nuevos deben ser 0 y la aceptacion verde:\n{lnote}")
                lok, lnote = gate_lint()
                if not (cok and lok):
                    write_supervision(f"ESCALATE_HUMAN: lint nuevo tras coder y Claude\n{lnote}")
                    return False, f"lint: {lnote[:200]}"
        mok, mnote = gate_mutacion()
        if not mok:
            return False, f"mutacion: {mnote}"
        sok, snote = gate_suite_total()
        if not sok and state.get("suite_total", {}).get("new_failures"):
            # D11: la regresion de suite iba directo a humano; ahora sigue la escalera (coder -> Claude -> humano)
            new = state["suite_total"]["new_failures"][:20]
            _, flog = sh([PY, "-m", "pytest", *new, "-q", "-x", "-p", "no:cacheprovider"])
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Tu cambio en {state['plan_files'][0]} rompio tests existentes que antes pasaban. Corregilo conservando "
                      f"la feature nueva y el comportamiento previo.\n\nTESTS ROTOS:\n{flog[-4000:]}\n\nARCHIVOS:\n{_joined(_all_code())}")
            code = one_file(strip_fence(out), state["plan_files"][0])
            if code:
                _write(state["plan_files"][0], code)
            aok, _ = accept()
            sok, snote = gate_suite_total() if aok else (False, "aceptacion rota tras el fix de regresion")
            if not sok:
                cok, _ = claude_fix(f"Tests existentes rotos por el cambio (deben volver a pasar sin perder la feature):\n{flog[-4000:]}")
                sok, snote = gate_suite_total() if cok else (False, "Claude no dejo verde la aceptacion")
        if not sok:
            write_supervision(f"ESCALATE_HUMAN: aceptacion verde pero suite total con regresion\n{snote}")
            return False, f"suite total: {snote[:200]}"
        rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas {ladder_note} + lint + suite total")
        return True, f"G4 + suite ok ({vueltas} vueltas {ladder_note})".strip()
    state["suite_total"] = test_counts(log)
    rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas {ladder_note}".strip())
    return True, f"G4 ok ({vueltas} vueltas {ladder_note})".strip()


@stage("5b-review")
def review():
    """Claude revisa el diff. Bloquea SOLO si deja un test_review.py que falla (ticket 07)."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    diff = subprocess.run(["git", "diff", "--cached", "--", *state["plan_files"]], cwd=WT,
                          capture_output=True, text=True, encoding="utf-8").stdout
    state["claude_calls"] += 1
    before = _tree()
    r = run_claude(
        "Revisa este diff contra docs/sdlc/spec.md y la TAREA. "
        f"Si encontras un defecto REAL, escribi UN test pytest que falle en {REVIEW_REL} "
        "(una funcion test_*) y responde 'BLOCK: <defecto>'. No toques ningun otro archivo. "
        "Si no hay defecto demostrable con test, no escribas nada y responde 'OK' o 'NOTE: <observacion>'.\n\n"
        f"TAREA:\n{TASK.task}\n\nDIFF:\n{diff[:60000]}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    verdict = (r.get("result") or "").strip()
    write_supervision(f"revision del diff (Claude, rc={r.get('returncode')}): {verdict[:2000]}")
    gate_alcance("diff-review-alcance", before, (REVIEW_REL,))
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
                    f"sdlc: {TASK_NAME} - pipeline 6 etapas, gates + escalera + revision Claude"], cwd=WT, check=True)
    base = state.get("base_sha", "HEAD~1")
    ds = subprocess.run(["git", "diff", "--stat", base], cwd=WT, capture_output=True, text=True).stdout
    state["diffstat"] = ds.strip().splitlines()[-1] if ds.strip() else ""
    ns = subprocess.run(["git", "diff", "--numstat", base, "--", *state["plan_files"]], cwd=WT, capture_output=True, text=True).stdout
    rows = [l.split("\t") for l in ns.splitlines() if l.count("\t") == 2 and l.split("\t")[0].isdigit()]
    state["lines"] = {"added": sum(int(a) for a, _, _ in rows), "deleted": sum(int(d) for _, d, _ in rows)}
    gate_lint()
    return True, state["diffstat"]


def gate_lint() -> tuple[bool, str]:
    """ruff + mypy sobre los archivos del plan. Bench: paquete nuevo, baseline 0. Repo: el hook exige 0."""
    cmd = CFG.get("lint_cmd")
    if cmd:  # otro lenguaje o linter propio del repo: exit 0 = limpio
        ok, log = sh(_cmd("lint_cmd", state["plan_files"]))
        state["lint_new"] = {"lint_cmd": 0 if ok else 1}
        return rec_gate("lint", ok, "0" if ok else log[-1500:])
    files = [f for f in state["plan_files"] if _es_py(f)]
    if not files:
        state["lint_new"] = {"ruff": 0, "mypy": 0}
        return rec_gate("lint", True, "sin archivos Python ni lint_cmd")
    _, rl = sh([sys.executable, "-m", "ruff", "check", "--output-format", "concise", *files])
    _, ml = sh([sys.executable, "-m", "mypy", "--ignore-missing-imports", *files])
    ruff = len(re.findall(r"^\S+:\d+:\d+: ", rl, re.M))
    mypy = int((re.search(r"Found (\d+) error", ml) or [0, 0])[1])
    state["lint_new"] = {"ruff": ruff, "mypy": mypy}
    detail = "0/0" if not (ruff or mypy) else (rl + "\n" + ml)[-1500:]
    return rec_gate("lint", not (ruff or mypy), detail)


def _seed_accept() -> None:
    """Escribe y commitea los tests de aceptacion del llamador (rojos por diseño) si no estan en el arbol."""
    accept = getattr(TASK, "accept_files", {})
    if not accept:
        return
    for rel, content in accept.items():
        if not (WT / rel).exists():
            _write(rel, content)
    subprocess.run(["git", "add", "--", *accept], cwd=WT, check=True)  # solo los tests: el resto del arbol no es nuestro
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=WT).returncode != 0:
        subprocess.run(["git", "-c", "user.name=mmorch", "-c", "user.email=mmorch@local", "commit", "-q", "-m",
                        f"sdlc: test de aceptacion {TASK_NAME} (rojo por diseño)"], cwd=WT, check=True)


def run(from_stage: float = 2) -> dict:
    """Corre las etapas desde `from_stage` (2 spec, 2.5 spec-review, 3 plan, 4 build, 5 test, 5.5 review, 6 pr).
    La etapa es el checkpoint (ticket 05 D4): reanudar = mismo worktree + from_stage."""
    _RUN_USD["usd"] = 0.0
    register_run_tracker(_RUN_USD)  # W3.4: tope USD por-run medido en proceso, no solo por el ledger
    try:
        return _run_stages(from_stage)
    finally:
        unregister_run_tracker(_RUN_USD)
        state["usd_run"] = round(_RUN_USD["usd"], 4)


def _run_stages(from_stage: float) -> dict:
    # D3 r1: el pre-commit de mmorch corre `python -m ruff` y resolvia al Python del sistema (sin ruff).
    os.environ["PATH"] = os.path.dirname(PY) + os.pathsep + os.environ.get("PATH", "")
    if not (WT / ".git").exists():
        WT.parent.mkdir(parents=True, exist_ok=True)
        if FEAT:
            br = f"sdlc/{PHASE}"
            exists = subprocess.run(["git", "-C", FEAT["repo"], "rev-parse", "--verify", "-q", br], capture_output=True).returncode == 0
            # D13/D14: un intento anterior dejo la branch creada -> `-b` falla (255). Se reusa la branch existente.
            args = ["worktree", "add", "-q", str(WT), br] if exists else ["worktree", "add", "-q", "-b", br, str(WT), "HEAD"]
            subprocess.run(["git", "-C", FEAT["repo"], *args], check=True)
        elif hasattr(TASK, "held_out"):  # bench.BenchTask
            bench.materialize(TASK, str(WT))
        else:  # directorio plano ya materializado por el llamador (workflow_race): el pipeline necesita git
            subprocess.run(["git", "init", "-q"], cwd=WT, check=True)
            subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
            subprocess.run(["git", "-c", "user.name=mmorch", "-c", "user.email=mmorch@local", "commit", "-q", "-m", "base"], cwd=WT)
        print("materializado", WT, flush=True)
    if not TASK.accept_files and LOG.exists():  # resume tras awaiting_approval: el test ya vive en la branch
        try:
            rel = json.loads(LOG.read_text(encoding="utf-8")).get("awaiting_approval")
            if rel and (WT / rel).exists():
                TASK.accept_files[rel] = (WT / rel).read_text(encoding="utf-8")
        except (OSError, ValueError):
            pass
    _seed_accept()  # tambien sobre un worktree abierto por el server: el test de aceptacion entra commiteado
    state["base_sha"] = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip() \
        if from_stage <= 3 else "HEAD~1"  # en resume no se conoce: HEAD~1 como antes
    gi = WT / ".gitignore"
    if not gi.exists():
        gi.write_text("__pycache__/\n", encoding="utf-8")  # r1-r3: la review hace git add -A antes del pr
    snapshot_baseline()
    state["t0"] = time.strftime("%Y-%m-%d %H:%M:%S")
    state["t0_epoch"] = time.time()
    if from_stage > 3:
        plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
        m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
        state["plan_files"] = _plan_files(m.group(1) if m else plan_md)
    for t in TPL.glob("*.md"):  # convencion por repo (ticket 11): las plantillas viajan con el repo
        _write(f"docs/sdlc/{t.name}", t.read_text(encoding="utf-8"))
    stages = {1: aceptacion, 2: spec, 2.5: spec_review, 3: plan, 4: build, 5: test, 5.5: review, 6: pr}
    try:
        for k in sorted(stages):
            if k >= from_stage:
                stages[k]()
                if k == 5.5 and state.get("review_block"):
                    test()  # una vuelta mas con el test de Claude; si sigue rojo, escala
    except StageFailed as e:  # la etapa es el checkpoint: el run-log dice donde reanudar
        state["failed_stage"], state["failed_note"] = e.stage, e.note
        print(str(e), flush=True)
    state["minutes_total"] = round((time.time() - state["t0_epoch"]) / 60, 2)
    state["usd"] = ledger_usd()
    _flush()
    here = RUNS / f"run-log-{PHASE}.json"
    here.write_text(LOG.read_text(encoding="utf-8"), encoding="utf-8")
    state["status"] = ("built" if not state.get("failed_stage") else "awaiting_approval" if state.get("awaiting_approval")
                       else "integration_failed" if str(state["failed_stage"]).startswith("5") else "escalate")
    print("SDLC TERMINO", json.dumps({k: state.get(k) for k in
          ("task", "calls", "minutes_total", "usd", "usd_by_family", "human_interventions", "claude_calls", "review_block",
           "escalated_to_claude", "gate_rejects", "diffstat", "lines", "suite_total", "lint_new")}, ensure_ascii=False))
    return dict(state)


def build_feature(name: str, task: str, repo: str, *, accept: dict[str, str] | None = None, accept_cmd: str | None = None,
                  files: list[str] | None = None, contract: list[str] | None = None, suite: list[str] | None = None,
                  wt=None, phase=None, from_stage: float | None = None, max_fix: int = 3, writer=None, coder=None) -> dict:
    """Construye UNA feature en un repo existente por el pipeline. Devuelve el estado final (run-log) con
    `status` in {built, integration_failed, escalate} y, si fallo, `failed_stage` (el checkpoint para reanudar).

    `accept` = {ruta relativa del test de aceptacion: contenido} (lo escribe el llamador, nunca el pipeline);
    sin `accept`, `accept_cmd` (el comando del repo) es el oraculo. `files` = lo que el plan puede escribir:
    `sdlc.toml` de la raiz del repo es el techo y un `files` del llamador solo lo acota (ticket 05 D2)."""
    toml = _toml(repo)
    accept_cmd = accept_cmd or toml.get("accept_cmd")
    if not accept and not accept_cmd:
        raise ValueError("build_feature necesita `accept` (tests) o `accept_cmd` (comando del repo / sdlc.toml)")
    techo = _resolve_files(repo, files)
    if from_stage is None:
        from_stage = 2 if accept else 1  # sin tests del llamador: la etapa 1 los escribe (ticket 12 D5)
    # `files` del llamador = obligatorios (el plan los lista si o si); el techo del toml = lo permitido
    feat = {"repo": repo, "task": task, "files": list(files or []), "techo": techo, "accept": dict(accept or {}), "contract": list(contract or []),
            "suite": suite or toml.get("suite") or ["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp",
                                                       str(pathlib.Path(tempfile.gettempdir()) / "pyt-sdlc-wt")]}
    t = types.SimpleNamespace(name=name, task=task, accept_files=dict(accept or {}))
    configure(t, contract=feat["contract"], feat=feat, wt=wt, phase=phase, max_fix=max_fix, writer=writer, coder=coder,
              accept_cmd=accept_cmd)
    return run(from_stage)


def _resolve_files(repo: str, files: list[str] | None) -> list[str]:
    """Devuelve el TECHO (patrones fnmatch de `files` en sdlc.toml). Un `files` del llamador solo acota: cada archivo
    tiene que caer dentro del techo; sin toml, el techo es exactamente lo que dice el llamador."""
    p = pathlib.Path(repo) / "sdlc.toml"
    techo = _files_from_toml(repo) if p.exists() else None
    if files is None:
        if techo is None:
            raise ValueError(f"sin `files` y sin {p}: el pipeline no sabe que archivos puede tocar (ticket 05 D2)")
        return techo
    if techo is not None:
        fuera = [f for f in files if not any(_calza(f, pat) for pat in techo)]
        if fuera:
            raise ValueError(f"files fuera del techo de {p}: {fuera} (el payload solo acota, no amplia)")
        return techo
    return list(files)


def _files_from_toml(repo: str) -> list[str]:
    import tomllib
    p = pathlib.Path(repo) / "sdlc.toml"
    if not p.exists():
        raise ValueError(f"sin `files` y sin {p}: el pipeline no sabe que archivos puede tocar (ticket 05 D2)")
    files = tomllib.loads(p.read_text(encoding="utf-8")).get("files") or []
    if not files:
        raise ValueError(f"{p} no declara `files`")
    return list(files)


def registrar_veredicto(repo: str, branch: str, label: str, motivo: str, task: str = "", test_rel: str = "") -> dict:
    """Ticket 08 D1/D2: cada aprobacion o rechazo del test de aceptacion deja un ejemplo etiquetado (test completo,
    etiqueta, motivo) en logs/sdlc/veredictos.jsonl. El test se lee de la branch: `test_rel` si el llamador lo sabe
    (modo grill: el test lo escribe un humano y no hay run-log), si no `docs/sdlc/run-log.json -> awaiting_approval`."""
    def _show(rel):
        r = subprocess.run(["git", "-C", repo, "show", f"{branch}:{rel}"], capture_output=True, text=True, encoding="utf-8", errors="replace")
        return r.stdout if r.returncode == 0 else ""
    rel = test_rel
    if not rel:
        try:
            rel = json.loads(_show("docs/sdlc/run-log.json") or "{}").get("awaiting_approval") or ""
        except ValueError:
            rel = ""
    rec = {"ts": time.time(), "repo": repo, "branch": branch, "test_rel": rel, "test": _show(rel) if rel else "",
           "label": label, "motivo": motivo, "task": task}
    RUNS.mkdir(parents=True, exist_ok=True)
    with (RUNS / "veredictos.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    return rec


def init(repo: str) -> dict:
    """Ticket 12: un repo entra al pipeline. Escribe `sdlc.toml` (techo = fuentes .py fuera de tests; accept_cmd; suite;
    approve_accept), `docs/sdlc/` con las plantillas y un puntero en AGENTS.md. Nunca pisa lo que ya existe."""
    root = pathlib.Path(repo).resolve()
    hecho: list[str] = []
    toml = root / "sdlc.toml"
    if not toml.exists():
        skip = {"tests", "tests_accept", ".venv", "venv", "node_modules", "build", "dist", "docs", "target", "__pycache__"}
        conteo: dict[str, int] = {}
        dirs: dict[str, set[str]] = {}
        ls = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, encoding="utf-8")
        # repo git: solo lo trackeado (Portfolio tenia _kimi_exp/ y _tmp_*.py sin trackear); si no, el arbol entero
        for q in ([root / x for x in ls.stdout.splitlines()] if ls.returncode == 0 and ls.stdout else root.rglob("*")):
            if not q.is_file():
                continue
            rel = q.relative_to(root)
            if any(part in skip or part.startswith(".") for part in rel.parts):
                continue
            ext = q.suffix.lstrip(".").lower()
            if ext in _LANG_HINT:
                conteo[ext] = conteo.get(ext, 0) + 1
                dirs.setdefault(ext, set()).add(rel.parts[0] if len(rel.parts) > 1 else "")
        exts = [e for e, _ in sorted(conteo.items(), key=lambda kv: -kv[1])][:3] or ["py"]
        globs = sorted({(f"{d}/**/*.{e}" if d else f"*.{e}") for e in exts for d in dirs.get(e, {""})})
        hints = "\n".join(f"# compile_cmd = \"{_LANG_HINT[e]}\"   # {e}" for e in exts if e != "py")
        toml.write_text(
            "# sdlc.toml — convencion por repo (ticket 11). `files` es el TECHO (patrones) de lo que el pipeline puede\n"
            "# escribir; un payload solo lo acota. Sin accept_cmd el repo no entra. approve_accept: un humano aprueba\n"
            "# el test de aceptacion que escribe la etapa 1 antes de gastar (ticket 12 D5).\n"
            "# Otros lenguajes: `ext`, `compile_cmd` (G3 y test-compile), `lint_cmd` ({files} = archivos del plan),\n"
            "# `suite_cmd` (suite total si no es pytest), `seed_globs` (ignorados que el worktree necesita), `accept_test` ({slug}).\n"
            + ('accept_cmd = "python -m pytest -q"\nsuite = ["tests", "-q", "-rfE", "-p", "no:cacheprovider"]\n'
               if (root / "tests").is_dir() else  # sin tests/, pytest en la raiz recolecta cualquier test_*.py (scrapers)
               '# accept_cmd = "<comando determinista, sin red>"   # sin tests/: definilo o el repo no entra\n')
            + f"ext = {json.dumps(exts)}\n" + (hints + "\n" if hints else "")
            + "approve_accept = true\nusd_max = 3.0\n"
            "files = [\n" + "".join(f'    "{g}",\n' for g in globs) + "]\n", encoding="utf-8")
        hecho.append("sdlc.toml")
    d = root / "docs" / "sdlc"
    d.mkdir(parents=True, exist_ok=True)
    for t in TPL.glob("*.md"):
        if not (d / t.name).exists():
            shutil.copy(t, d / t.name)
            hecho.append(f"docs/sdlc/{t.name}")
    ag = root / "AGENTS.md"
    marca = "## SDLC (pipeline de 6 etapas)"
    if ag.exists() and marca not in ag.read_text(encoding="utf-8"):
        ag.write_text(ag.read_text(encoding="utf-8").rstrip("\n") + f"\n\n{marca}\n\nEste repo construye features con "
                      "`mmorch.sdlc` (skill `/project`). Contrato del repo en `sdlc.toml`; artefactos de cada corrida en "
                      "`docs/sdlc/` de la review branch.\n", encoding="utf-8")
        hecho.append("AGENTS.md")
    return {"repo": str(root), "hecho": hecho}


def main(argv: list[str] | None = None) -> int:
    """CLI: `python -m mmorch.sdlc init <repo>` | `--task <bench> [--wt DIR] [--phase P] [--from-stage N]`."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["init"]:
        print(json.dumps(init(argv[1] if len(argv) > 1 else "."), ensure_ascii=False))
        return 0

    def arg(flag, default):
        return argv[argv.index(flag) + 1] if flag in argv else default

    for _s in (sys.stdout, sys.stderr):  # D10: la consola cp1252 tiro UnicodeEncodeError por un "->" de Claude
        getattr(_s, "reconfigure", lambda **k: None)(encoding="utf-8", errors="replace")
    name = arg("--task", "rate-limiter")
    task = bench.get_task(name)
    configure(task, contract=BENCH_CONTRACTS.get(name, []), wt=arg("--wt", None), phase=arg("--phase", None),
              max_fix=arg("--max-fix", 3))
    if "--self-check" in argv:
        return self_check()
    return 0 if run(float(arg("--from-stage", "2"))).get("status") == "built" else 1


# Tokens que la spec tiene que nombrar verbatim (G1) para las tasks del bench.
BENCH_CONTRACTS = {
    "rate-limiter": ["TokenBucket", "MultiLimiter", "allow", "capacity", "refill_per_s", "now", "key",
                     "limiter/core.py", "limiter/multi.py", "limiter/__init__.py"],
    "etl-pipeline": ["parse_lines", "normalize", "to_summary", "run", "maxsplit", "count", "total", "by_name",
                     "etl/extract.py", "etl/transform.py", "etl/load.py", "etl/__init__.py"],
    "lru-ttl-cache": ["LRUCache", "maxsize", "ttl_s", "get", "put", "now", "None", "cache/core.py", "cache/__init__.py"],
}


if __name__ == "__main__":
    sys.exit(main())
