"""sdlc — pipeline de 6 etapas con gates (spec -> plan -> build -> test -> review -> pr). Ticket 05, paso 1.

Reemplaza a project_integrate.build_project como engine de /project. Validado 6/6 en el bench y D2..D15 sobre
mmorch (research/07-resultados.md del mapa sdlc-6-gates). Gates deterministas (contrato, allowlist, trazabilidad,
compile, test-compile, cambio-minimo, sin-clones, docstring, suite total por nombre, lint, alcance) y escalera
fix loop 3 -> reasoner x2 -> Claude (edit) -> humano, en la etapa 4 (exito = compila) y en la 5 (exito = aceptacion).
Si falta el compilador, la etapa 4 va directo a Claude para el entorno. G3 y test-compile descuentan los errores que la
base ya tenia sin la aceptacion propia (tests rojos por diseño de otras features). La etapa es el checkpoint: run-log.json + from_stage.

Uso como biblioteca: `build_feature(name, task, repo, accept={rel: contenido}, files=[...], contract=[...])`.
Uso como CLI: `python -m sdlc init <repo>` (adopcion). El bench de mmorch lo corre como consumidor (`mmorch.sdlc_bench`).
"""
from __future__ import annotations

import hashlib
import itertools
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import types
from typing import Any

from mmorch.paths import logs_dir
from mmorch.providers import call, register_run_tracker, unregister_run_tracker

from . import areas, propiedades

# Globals de corrida: los fija configure() (antes venian de argv). Ninguna funcion los lee antes de configure().
TASK_NAME = ""
WT = pathlib.Path(".")
PHASE = ""
FEAT: dict | None = None
TASK: Any = None  # SimpleNamespace(name, task, accept_files) o una task del bench de mmorch
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
TPL = pathlib.Path(__file__).resolve().parent / "templates"  # spec-kit recortado (tickets 02 y 11)
_ORCH = pathlib.Path(__file__).resolve().parents[1]   # bd de respaldo para los archivos fuera de area (ticket 01)


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
    d.update(_toml(WT))  # docs/sdlc/sdlc.toml ya no se lee: pisaba la raiz sin aviso (repos-convenciones 03)
    return d


def _repo_python() -> str:
    """Interprete del repo (ticket 12): `python` de sdlc.toml, o el venv sembrado en el worktree (el server siembra
    .venv/venv), o el de mmorch. Sin esto la suite de un repo con dependencias propias corre con el Python equivocado."""
    return _python_de(WT, CFG) or sys.executable


def _python_de(root, cfg: dict) -> str | None:
    """Regla unica del interprete de un repo: la usan el pipeline y `sdlc propiedades` (repos-convenciones 09)."""
    for c in ([cfg["python"]] if cfg.get("python") else []) + [
            ".venv/Scripts/python.exe", ".venv/bin/python", "venv/Scripts/python.exe", "venv/bin/python"]:
        if (pathlib.Path(root) / c).exists():
            return str(pathlib.Path(root) / c)
    return None


CFG: dict = {}
state: dict = {}
_RUN_LOCK = threading.Lock()
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
    """Fija la corrida. `task` tiene .name, .task y .accept_files ({rel: contenido}); una task del bench sirve tal cual.
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
    if FEAT:  # 2026-09-17: un nombre fijo hacia chocar la revision de dos features al mergear; el slug sale del test
        acc = next(iter(task.accept_files), TASK_NAME)  # de aceptacion, estable al reanudar la misma feature
        slug = re.sub(r"^test_(sdlc_)?|_test$", "", re.sub(r"[^a-z0-9]+", "_", pathlib.Path(acc).name.split(".")[0].lower()))
        REVIEW_REL = f"{TESTS_PREFIX}test_review_{slug.strip('_')[:40] or 'sdlc'}.py"
    WRITER = writer or os.environ.get("SDLC_WRITER", "deepseek-reasoner")
    CODER = coder or os.environ.get("SDLC_CODER", "deepseek-v4-pro")
    DIAG = diag or os.environ.get("SDLC_DIAG", WRITER)
    CFG = _cfg()
    PY = _repo_python()
    state.clear()
    state.update({"proto": "sdlc", "task": TASK_NAME, "coder": CODER, "stages": [], "gates": [], "calls": 0,
                  "human_interventions": 0, "claude_calls": 0, "gate_rejects": [], "escalated_to_claude": False})
    SNAP.clear(); SEEN.clear(); _orig_files.clear(); _COMPILE_BASE.clear()
    RUNS.mkdir(parents=True, exist_ok=True)


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    if str(model).startswith("claude:"):   # builder-e2e ticket 10: Haiku como coder por la CLI (la familia anthropic no es un nodo)
        env = {k: v for k, v in os.environ.items() if k not in _SIN_SHELL}
        texto, usd = _claude_texto(model.split(":", 1)[1], user, env, timeout, tools="", rol="coder", system=system)
        state["claude_coder_usd"] = round(state.get("claude_coder_usd", 0.0) + usd, 4)
        return texto
    try:
        r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
                 pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
    except RuntimeError as e:  # D13: el reasoner agoto 32k tokens razonando -> un intento con el coder
        # "respuesta vac" sin la vocal final: providers escribe "vacía" con tilde y el match exacto nunca disparaba
        if "respuesta vac" not in str(e) or model == CODER:
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


_SIN_SHELL = ("CLAUDECODE", "SHELL", "BASH", "BASH_ENV", "MSYSTEM")   # cursor-agent rompe los hooks con el bash de Git


def _cursor_argv() -> list[str]:
    from mmorch.cursor_worker import cursor_argv   # una sola fuente: el despachador de Cursor (orchestration-ayz)
    return cursor_argv()


def _agent_run(argv: list[str], prompt: str | None, env: dict, timeout: float) -> tuple[int, str]:
    """Corre un agente en el worktree (timeout o rc != 0 = falla tecnica, ver cursor_worker.run_agent)."""
    from mmorch.cursor_worker import run_agent
    return 0, run_agent(argv, WT, prompt, env, timeout)


def _claude_texto(model: str, prompt: str, env: dict, timeout: float, *, tools: str, rol: str,
                  system: str | None = None) -> tuple[str, float]:
    """`claude -p` headless, aislado (--restricted ignora los settings y hooks del usuario; sin MCP ni sesion).
    Devuelve (texto, US$ equivalentes de cupo). `tools=""` = sin herramientas: solo texto (rol coder)."""
    from mmorch.claude_exec import claude_bin
    argv = claude_bin() + ["-p", "--model", model, "--restricted", "--strict-mcp-config", "--tools", tools,
                           "--no-session-persistence", "--output-format", "json"]   # el prompt va por stdin: --tools es variadico
    if system:
        argv += ["--system-prompt", system]
    _, out = _agent_run(argv, prompt, env, timeout)
    try:
        d = json.loads((out.strip().splitlines() or ["{}"])[-1])
    except ValueError:
        raise RuntimeError(f"{rol} claude: salida sin JSON: {out[-300:]}") from None
    if d.get("is_error") or not d.get("result"):   # cupo agotado, auth: falla tecnica, no un resultado malo
        raise RuntimeError(f"{rol} claude: {str(d.get('result') or d)[:300]}")
    return str(d["result"]), float(d.get("total_cost_usd") or 0.0)


def _planner(system: str, user: str) -> str:
    """Planner de la etapa 3 (orchestration-34k). SDLC_PLANNER vacio = el WRITER, una llamada sin ver el repo.
    `claude:<modelo>` y `cursor:<modelo>` corren un agente de SOLO LECTURA en el worktree, sin MCP ni hooks."""
    p = os.environ.get("SDLC_PLANNER", "") or WRITER
    if not p.startswith(("claude:", "cursor:")):
        return llm(p, system, user)
    kind, model = p.split(":", 1)
    prompt = (f"{system}\n\nExplora el repo de este directorio con tus herramientas de lectura antes de planear. No edites "
              f"ningun archivo. Tu respuesta final es SOLO el contenido de plan.md.\n\n{user}")
    env = {k: v for k, v in os.environ.items() if k not in _SIN_SHELL}
    timeout = float(CFG.get("planner_timeout_s", 1200))
    state["calls"] += 1
    state["planner"] = p
    if kind == "claude":
        texto, usd = _claude_texto(model, prompt, env, timeout, tools="Read,Grep,Glob", rol="planner")
        state["planner_usd"] = round(state.get("planner_usd", 0.0) + usd, 4)
        return texto
    cfg, cli = WT / ".cursor", WT / ".cursor" / "cli.json"
    nuevo = not cfg.exists()   # el repo puede versionar su propio .cursor/ (reglas): la limpieza solo deshace lo nuestro
    previo = cli.read_bytes() if cli.exists() else None
    cfg.mkdir(exist_ok=True)   # sin interfaz Cursor ve Gmail/Calendar/Drive de sus plugins: deny Mcp(*:*) (medido 2026-10-01)
    cli.write_text(json.dumps({"permissions": {"allow": [], "deny": ["Mcp(*:*)"]}}), encoding="utf-8")
    try:
        _, out = _agent_run(_cursor_argv() + ["-p", "--trust", "--mode", "plan", "--workspace", str(WT), "--model", model,
                                              "--output-format", "stream-json", prompt], None, env, timeout)
    finally:
        if nuevo:
            shutil.rmtree(cfg, ignore_errors=True)
        elif previo is None:
            cli.unlink(missing_ok=True)
        else:
            cli.write_bytes(previo)
    plan_txt, result = "", ""
    for line in out.splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "result":
            result = str(ev.get("result") or "")
            u = ev.get("usage") or {}
            state["planner_tokens"] = state.get("planner_tokens", 0) + (u.get("inputTokens") or 0) + (u.get("outputTokens") or 0)
        elif ev.get("type") == "tool_call" and "createPlanToolCall" in (ev.get("tool_call") or {}):
            plan_txt = str((ev["tool_call"]["createPlanToolCall"].get("args") or {}).get("plan") or plan_txt)
    if not (plan_txt or result):
        raise RuntimeError(f"planner cursor: sin plan ni result: {out[-300:]}")
    return plan_txt or result   # --mode plan deja el plan en createPlan, no en result


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
        return _tipo_propio(code, rel)
    secs = {parts[i]: parts[i + 1].strip() for i in range(1, len(parts), 2)}
    rec_gate("un-archivo", rel in secs, f"{rel}: recorte de {list(secs)}")
    return _tipo_propio(secs.get(rel, code), rel)


# ponytail: solo tipos de primer nivel en columna 0 (el estilo de javac y de los formateadores); uno indentado no cuenta
_TIPO_PUBLICO = re.compile(r"(?m)^public\s+(?:(?:abstract|final|sealed|non-sealed|strictfp)\s+)*"
                           r"(?:class|interface|enum|record|@interface)\s+(\w+)")


def _tipo_propio(code: str, rel: str) -> str:
    """orchestration-7b6: en Java el tipo publico tiene el nombre del archivo. Pedido Opcion.java, el coder devolvio la
    clase Negocio (y Opcion en Bot.java, 519 -> 101 lineas): ese codigo no se escribe. Devuelve "" y el que llama
    conserva el archivo."""
    if not rel.endswith(".java"):
        return code
    ajenos = [t for t in _TIPO_PUBLICO.findall(code) if t != pathlib.Path(rel).stem]
    if ajenos:
        rec_gate("tipo-del-archivo", False, f"{rel}: declara public {ajenos[0]}")
        return ""
    return code


def sh(cmd, timeout: float | None = None, keep: int = 6000, *, host: bool = False, mounts: tuple[str, ...] = ()):
    """Lista = exec directo; str = comando shell (accept_cmd del repo). `python` en un comando = el interprete del repo.
    Con `contenedor = "<imagen>"` en sdlc.toml el comando corre en esa imagen, sin red (orchestration-s8w); `host=True`
    lo deja afuera (herramientas de mmorch que no ejecutan codigo generado). `mounts` = rutas extra que el comando lee
    o escribe fuera del worktree."""
    # PYTHONDONTWRITEBYTECODE: dos mutantes del mismo tamaño escritos en el mismo segundo reusaban el .pyc del anterior
    # y el mutante "sobrevivia" (medido en Adepor 2026-09-15: 2 falsos vivos de 7).
    env = {**os.environ, "PATH": os.path.dirname(PY) + os.pathsep + os.environ.get("PATH", ""), "PYTHONDONTWRITEBYTECODE": "1"}
    texto = (cmd if isinstance(cmd, str) else " ".join(cmd))[:200]
    img = None if host else CFG.get("contenedor")
    eng: str | None = None
    name = ""
    if img:
        from mmorch.sandbox import container_argv, engine
        eng = engine()
        if not eng:
            raise StageFailed("contenedor", f"sdlc.toml pide la imagen {img}, pero no hay docker ni podman en PATH")
        if isinstance(cmd, list) and cmd and cmd[0] == PY:
            cmd = ["python", *cmd[1:]]  # el venv del host no corre en la imagen: el interprete es el de la imagen
        name = f"sdlc-{PHASE}-{os.getpid()}-{time.time_ns()}"
        # seed_globs enlaza dirs del repo principal (node_modules, caches): sin montarlo, el symlink queda roto adentro
        ro = [str(FEAT["repo"])] if FEAT and FEAT.get("repo") and CFG.get("seed_globs") else []
        cmd = container_argv(cmd, image=img, cwd=str(WT), mounts=[str(WT), *mounts], name=name, eng=eng, ro=ro)
    try:
        p = subprocess.run(cmd, cwd=WT, capture_output=True, text=True, encoding="utf-8", errors="replace",
                           timeout=timeout or CFG["cmd_timeout_s"], shell=isinstance(cmd, str), env=env)
    except subprocess.TimeoutExpired:
        if img and eng:  # matar el cliente no para el contenedor: sin esto queda corriendo
            subprocess.run([eng, "rm", "-f", name], capture_output=True)
        return False, f"TIMEOUT {timeout or CFG['cmd_timeout_s']}s: {texto}"
    return p.returncode == 0, (p.stdout + p.stderr)[-keep:]


_LANG_NAMES = {".py": "Python", ".ts": "TypeScript", ".tsx": "TypeScript", ".js": "JavaScript", ".jsx": "JavaScript",
               ".mjs": "JavaScript", ".java": "Java", ".kt": "Kotlin", ".go": "Go", ".rs": "Rust", ".cs": "C#",
               ".cpp": "C++", ".c": "C", ".rb": "Ruby", ".php": "PHP", ".swift": "Swift",
               # builder-e2e: sin estas, index.html caia al lenguaje del plan y el coder escribia TypeScript adentro
               ".html": "HTML", ".css": "CSS"}
# Salidas de un comando que no existe (shell de Windows en ingles y castellano, POSIX, y avisos de npx/tsc).
_NO_TOOL = re.compile(r"is not recognized as an internal or external command|no se reconoce como un comando"
                      r"|command not found|: not found\b|To get access to the TypeScript compiler"
                      r"|could not determine executable to run|El sistema no puede encontrar el archivo especificado", re.I)


def _rol(f: str | None = None) -> str:
    """Rol del coder armado desde el plan (orchestration-7ys: antes decia Python siempre, tambien en repos TypeScript
    y Java, que son justo donde el build cortaba por compilacion). Lenguaje del archivo `f` (o el mayoritario del
    plan) y el stack que declara la seccion `## Stack` del plan."""
    from collections import Counter
    lang = _LANG_NAMES.get(pathlib.Path(f).suffix) if f else None
    if not lang:
        c = Counter(_LANG_NAMES.get(pathlib.Path(x).suffix) for x in state.get("plan_files", []))
        c.pop(None, None)
        lang = c.most_common(1)[0][0] if c else _lenguaje()
    stack = state.get("plan_stack")
    tipo = f"En {f} el unico tipo publico se llama {pathlib.Path(f).stem}. " if f and f.endswith(".java") else ""
    return f"Sos un programador {lang} senior" + (f" (stack del plan: {stack})" if stack else "") + ". " + tipo + _notas()


def _notas() -> str:
    """orchestration-4rf: SDLC_NOTAS = archivo con notas de campo de corridas previas del repo (brazo N1). Sin la
    variable, el prompt del coder queda identico. Bloque congelado en .scratch/builder-e2e/research/4rf-preregistro.md §7.5."""
    p = os.environ.get("SDLC_NOTAS")
    if not p:
        return ""
    return ("\nNotas de campo de corridas anteriores sobre este repo (de la más reciente a la más vieja):\n<<<\n"
            + pathlib.Path(p).read_text(encoding="utf-8").strip()
            + "\n>>>\nUsalas si aplican. Pueden no aplicar a este archivo.\n")


def _constitucion() -> str:
    """orchestration-fcr: reglas fijas del repo destino (sdlc.toml `constitution`, default GOAL.md) para spec y plan,
    como la constitucion de Spec Kit. Sin el archivo, el prompt queda identico."""
    p = WT / str(CFG.get("constitution", "GOAL.md"))
    if not p.is_file():
        return ""
    return (f"\n\nCONSTITUCION DEL REPO ({p.name}; reglas fijas: la spec y el plan no las contradicen):\n"
            + p.read_text(encoding="utf-8").strip()[:6000])


def _lenguaje() -> str:
    """Lenguaje principal del repo: la primera `ext` de sdlc.toml (py si no declara)."""
    return _LANG_NAMES.get("." + _exts()[0], _exts()[0])


def _plan_stack(plan_md: str) -> str:
    """Seccion `## Stack` del plan en una linea: lenguaje, frameworks y librerias que usa el cambio."""
    m = re.search(r"## Stack\b(.*?)(?:\n## |\Z)", plan_md or "", re.S | re.I)
    return " ".join(m.group(1).split()).strip(" -*:")[:200] if m else ""


def _toolchain_missing(log: str) -> bool:
    """El compilador del repo no esta instalado o no esta en PATH, o falta una dependencia del entorno: las vueltas
    del coder no pueden arreglarlo."""
    return bool(_NO_TOOL.search(log or "")) or _falta_del_entorno(log) is not None


# Dependencia ausente (tsc, Node, Python, Maven y cargo sin red). El nombre, si lo hay, queda en el grupo `n`.
_NO_DEP = re.compile(r"TS2307: Cannot find module '(?![./])(?P<n>[^']+)'|TS2688: Cannot find type definition file"
                     r"|ERR_MODULE_NOT_FOUND\]: Cannot find package '(?P<n2>[^']+)'"
                     r"|ModuleNotFoundError: No module named '(?P<n3>[^'.]+)"
                     r"|Could not resolve dependencies for project|Non-resolvable (?:parent|import) POM"
                     r"|no matching package named `[^`]+`|failed to get `[^`]+` as a dependency")
_MANIFIESTOS = ("package.json", "pyproject.toml", "requirements.txt", "requirements-dev.txt", "Cargo.toml", "pom.xml")


def _falta_del_entorno(log: str) -> str | None:
    """orchestration-7ys / ticket 07 de builder-e2e: la linea que muestra una dependencia que el ENTORNO no trae.
    Cuenta un builtin (`node:test`, tipos de Node) o un paquete DECLARADO en un manifiesto del repo; Maven y cargo
    sin red solo fallan asi con dependencias declaradas. Un paquete que nadie declaro es un error del codigo: el coder
    lo invento. Medido 2026-10-15: TS2307 de `node:test` costo 3 vueltas del coder y 2 escaladas a Claude en vano."""
    manifiestos = None
    for m in _NO_DEP.finditer(log or ""):
        nombre = m.group("n") or m.group("n2") or m.group("n3")
        if not nombre or nombre.startswith("node:"):
            return m.group(0)
        if manifiestos is None:
            manifiestos = " ".join((WT / f).read_text(encoding="utf-8", errors="replace")
                                   for f in _MANIFIESTOS if (WT / f).is_file())
        raiz = "/".join(nombre.split("/")[:2]) if nombre.startswith("@") else nombre.split("/")[0]   # @scope/pkg o pkg
        if re.search(rf"[\"'\s]{re.escape(raiz)}[\"'\s=<>~^\[]", manifiestos):
            return m.group(0)
    return None


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


def _mutantes_texto(code: str, max_n: int = 8, lineas: set[int] | None = None) -> list[str]:
    """Mutantes para lenguajes sin AST en stdlib (Rust, JS, Go, C...): un swap de operador/booleano por mutante,
    solo en lineas de codigo (no comentarios ni imports). Un mutante que no compila se descarta (mortinato)."""
    lines = code.split("\n")
    out: list[str] = []
    for i, line in enumerate(lines):
        st = line.strip()
        if (lineas is not None and i + 1 not in lineas) or not st or st.startswith(("//", "#", "*", "/*", "use ", "import ", "require(", "mod ", "package ")):
            continue
        for a, b in _MUT_TEXTO:
            if a in line:
                m = lines[:i] + [line.replace(a, b, 1)] + lines[i + 1:]
                out.append("\n".join(m))
                if len(out) >= max_n:
                    return out
    return out


def _accept_mata() -> tuple[bool, list[str] | None]:
    """(mutante muerto = la aceptacion FALLA, tests que fallaron o None si el runner no da JUnit). Sin -x: una
    propiedad corre aunque el ejemplo falle primero (ticket 09). Dentro de los mutantes, el perfil de 20 ejemplos."""
    files = _accept_paths() if TASK.accept_files and all(_es_py(f) for f in TASK.accept_files) else []
    ok, _, res = _corre_con_junit(files, mut=True)
    return not ok, (sorted(t for t, paso in res.items() if not paso) if res is not None else None)


def _corre_con_junit(files: list[str], *, mut: bool = False) -> tuple[bool, str, dict[str, bool] | None]:
    """Corre `files` con pytest, o el comando de aceptacion del repo, con reporte JUnit (ticket 10: un solo parser).
    Devuelve (ok, log, {test: paso}); None = el runner no da JUnit."""
    d = pathlib.Path(tempfile.mkdtemp(prefix="sdlc-junit-"))
    xml = d / "junit.xml"
    if files and all(_es_py(f) for f in files):
        ok, log = sh([PY, "-m", "pytest", *files, "-q", "-p", "no:cacheprovider", f"--junitxml={xml}", *_prop_args(mut)],
                     mounts=(str(d),))
    else:
        base = _cmd_prop(str(ACCEPT_CMD or CFG.get("accept_cmd") or "false"), mut)
        ok, log = sh(propiedades.con_junit(base, str(xml)) or base, mounts=(str(d),))
    res = propiedades.junit(xml) if xml.is_file() else None
    shutil.rmtree(d, ignore_errors=True)
    return ok, log, res


def _prop_args(mut: bool = False) -> list[str]:
    """pytest carga el perfil de Hypothesis (archivo auxiliar) con la semilla del sha base (ticket 10)."""
    p = state.get("propiedades") or {}
    if p.get("libreria") != "hypothesis":
        return []
    aux = propiedades.AUX_PY_MUT if mut else propiedades.AUX_PY
    return ["-p", aux.removesuffix(".py"), f"--hypothesis-seed={p['semilla']}"]


def _cmd_prop(cmd: str, mut: bool = False) -> str:
    """`node ...` carga el perfil de fast-check con --import (ticket 10)."""
    return propiedades.con_aux_node(cmd, mut) if (state.get("propiedades") or {}).get("libreria") == "fast-check" else cmd


def _preparar_propiedades() -> None:
    """Ticket 10: la libreria se detecta por codigo (Hypothesis en Python, fast-check en TypeScript/JavaScript). Sin
    libreria, la corrida sigue sin propiedades y el run-log lo anota. Con libreria, semilla del sha base y perfil en
    archivos auxiliares del worktree, ignorados por git (.git/info/exclude): el repo no cambia."""
    lang, seed = _lenguaje(), propiedades.semilla(_sha_corrida())
    falta = {"Python": "hypothesis", "TypeScript": "fast-check", "JavaScript": "fast-check"}.get(lang)
    if falta == "hypothesis":
        lib = falta if sh([PY, "-c", "import hypothesis"])[0] else None
    else:
        lib = falta if falta and (WT / "node_modules" / "fast-check").exists() else None
    state["propiedades"] = {"libreria": lib, "semilla": seed}
    if not lib:
        state["propiedades"]["nota"] = (f"sin {falta}: la corrida sigue sin propiedades (adopcion: orchestration-4u6)"
                                        if falta else f"{lang}: sin libreria de propiedades")
        return
    nombres = propiedades.escribir_aux(WT, seed, lib)
    ex = subprocess.run(["git", "rev-parse", "--git-path", "info/exclude"], cwd=WT, capture_output=True, text=True).stdout.strip()
    if ex:
        p = WT / ex
        p.parent.mkdir(parents=True, exist_ok=True)
        previo = p.read_text(encoding="utf-8") if p.exists() else ""
        nuevos = [n for n in nombres if n not in previo.split()]
        if nuevos:
            p.write_text(previo + ("" if previo.endswith("\n") or not previo else "\n") + "\n".join(nuevos) + "\n", encoding="utf-8")


def _sha_corrida() -> str:
    """Sha corto del commit base de la corrida: semilla de las propiedades y nombre del delta archivado."""
    sha = state.get("sha_corrida") or state.get("base_sha") or "HEAD"
    return subprocess.run(["git", "rev-parse", "--short", sha], cwd=WT, capture_output=True, text=True).stdout.strip() or sha


_COBERTURA = r'''
import json, os, sys
destino, archivos, args = sys.argv[1], {os.path.normcase(os.path.abspath(a)) for a in json.loads(sys.argv[2])}, sys.argv[3:]
M = sys.monitoring
corridas = set()
def linea(code, n):
    f = os.path.normcase(code.co_filename)
    if f in archivos:
        corridas.add((f, n))
    return M.DISABLE  # cada linea avisa una sola vez: el costo no escala con la suite
M.use_tool_id(M.COVERAGE_ID, "sdlc")
M.register_callback(M.COVERAGE_ID, M.events.LINE, linea)
M.set_events(M.COVERAGE_ID, M.events.LINE)
import pytest
rc = pytest.main(args)
out = {}
for f in archivos:
    pend, ejec = [compile(open(f, encoding="utf-8").read(), f, "exec")], set()
    while pend:
        c = pend.pop()
        ejec.update(n for _, _, n in c.co_lines() if n)
        pend.extend(k for k in c.co_consts if hasattr(k, "co_lines"))
    out[f] = {"ejecutables": sorted(ejec), "corridas": sorted(n for g, n in corridas if g == f)}
json.dump(out, open(destino, "w", encoding="utf-8"))
sys.exit(rc)
'''


def _cobertura(files: list[str]) -> dict[str, tuple[int, list[int]]] | None:
    """{archivo: (lineas ejecutables, lineas que la aceptacion NO corre)} con el interprete del repo (sys.monitoring,
    3.12+). None = no se pudo medir (interprete viejo o la aceptacion no corre)."""
    destino = pathlib.Path(tempfile.mkdtemp()) / "cobertura.json"
    ok, _ = sh([PY, "-c", _COBERTURA, str(destino), json.dumps([str(WT / f) for f in files]),
                *_accept_paths(), "-q", "-p", "no:cacheprovider", *_prop_args()], mounts=(str(destino.parent),))
    if not ok or not destino.exists():
        return None
    datos = json.loads(destino.read_text(encoding="utf-8"))
    if CFG.get("contenedor"):  # las claves salen del contenedor: en Windows son /c/..., sin normcase
        from mmorch.sandbox import ruta_contenedor
        clave = lambda p: ruta_contenedor(os.path.abspath(p))  # noqa: E731
    else:
        clave = lambda p: os.path.normcase(os.path.abspath(p))  # noqa: E731
    out = {}
    for f in files:
        d = datos[clave(WT / f)]
        out[f] = (len(d["ejecutables"]), sorted(set(d["ejecutables"]) - set(d["corridas"])))
    return out


def _en_head(f: str) -> bool:
    return subprocess.run(["git", "cat-file", "-e", f"HEAD:{f}"], cwd=WT, capture_output=True).returncode == 0


def _lineas_cambiadas(f: str) -> set[int] | None:
    """Lineas (1-based) de `f` que difieren de HEAD. None = archivo nuevo: todas cuentan."""
    if not _en_head(f):
        return None
    diff = subprocess.run(["git", "diff", "-U0", "HEAD", "--", f], cwd=WT, capture_output=True, text=True,
                          encoding="utf-8", errors="replace").stdout
    return {n for a, b in re.findall(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@", diff, re.M) for n in range(int(a), int(a) + int(b or 1))}


def gate_sprites() -> tuple[bool, str]:
    """Ticket 15: los assets del repo pasan la capa DETERMINISTA (bloquea) y, si hay referencias, el juez visual deja
    su opinion EN SOMBRA (nunca bloquea: un juez VLM ordena pero no puntua, medido 2026-09-17)."""
    cfg = CFG.get("sprites")
    if not cfg:
        return rec_gate("sprites", True, "el repo no declara [sprites]")
    from .sprites import juez_visual, revisar
    res = revisar(cfg, WT)
    malos = [f"{n}: {d}" for n, ok, d in res if not ok]
    base = WT / cfg.get("dir", ".")
    for rel, ref in (cfg.get("referencias") or {}).items():
        try:
            juez_visual(base / rel, base / ref)
        except Exception as e:  # noqa: BLE001 — la sombra nunca puede tumbar una corrida
            rec_gate("juez-visual-sombra", True, f"{rel}: no se pudo consultar ({str(e)[:100]})")
    if cfg.get("referencias"):
        from .sprites import confiable
        try:
            v = [json.loads(x) for x in (RUNS / "veredictos.jsonl").read_text(encoding="utf-8").splitlines() if x.strip()]
        except OSError:
            v = []
        rec_gate("juez-visual-sombra", True, confiable(v)[1])  # informativo: cuanto falta para confiar en el juez
    state["sprites"] = {"chequeos": len(res), "malos": len(malos)}
    return rec_gate("sprites", not malos, "; ".join(malos[:5]) or f"{len(res)} chequeos verdes")


def gate_codigo_muerto() -> tuple[bool, str]:
    """2026-09-17: ningun gate medía codigo muerto en archivos NUEVOS (cambio-minimo solo mira los existentes) y
    macro_leadlag llego con ~180 de 333 lineas para formatos que walk_forward_corr nunca devuelve. Mide que fraccion
    de las lineas ejecutables de los .py nuevos del plan corre la aceptacion; bloquea bajo `cobertura_min`.
    Default 0.8 medido: esa version corria 0.533 y la recortada 0.947 (las firmas multilinea no siempre avisan)."""
    nuevos = [f for f in state.get("plan_files", []) if _es_py(f) and (WT / f).exists() and not _en_head(f)]
    if not nuevos or not TASK.accept_files:
        return rec_gate("codigo-muerto", True, "sin .py nuevos o sin tests de aceptacion pytest: no mide")
    medida = _cobertura(nuevos)
    if medida is None:
        return rec_gate("codigo-muerto", True, f"no se pudo medir con {PY} (sys.monitoring pide Python 3.12+)")
    minimo = float(CFG.get("cobertura_min", 0.8))
    state["cobertura_nueva"] = {f: round(1 - len(falta) / max(total, 1), 3) for f, (total, falta) in medida.items()}
    bajos = {f: medida[f][1] for f, c in state["cobertura_nueva"].items() if c < minimo}
    detalle = "; ".join(f"{f}: {state['cobertura_nueva'][f]} corre, lineas sin ejecutar {falta}" for f, falta in bajos.items())
    return rec_gate("codigo-muerto", not bajos, detalle or f"cobertura de archivos nuevos {state['cobertura_nueva']} >= {minimo}")


def gate_mutacion() -> tuple[bool, str]:
    """Ticket 08 D2 (opcion D): el test de aceptacion mata mutantes del codigo final. Score = muertos / total sobre los
    archivos del plan (mutantes de checkers._mutants: operadores, comparaciones, booleanos, enteros). Bloquea SOLO con
    `mutation_min` en sdlc.toml; sin numero medido, observa y deja state["mutation_score"]."""
    from mmorch.checkers import _mutants
    if not TASK.accept_files and not (ACCEPT_CMD or CFG.get("accept_cmd")):
        return rec_gate("mutacion", True, "sin oraculo de aceptacion: sin mutacion")
    killed = total = 0
    compile_cmd = CFG.get("compile_cmd")
    minimo = CFG.get("mutation_min")
    # orchestration-cbr: cada mutante corre la aceptacion entera (en Java y TS, la suite) y la etapa 5 tardaba 10-30
    # min por un puntaje que solo observa (ningun repo fija mutation_min). Sin minimo: una muestra repartida entre
    # los archivos. Con minimo, el gate bloquea y prueba todos.
    tope = None if minimo is not None else int(CFG.get("mutation_muestra", 5))
    por_archivo = []
    for f in state.get("plan_files", []):
        if not (WT / f).exists():
            continue
        orig = (WT / f).read_text(encoding="utf-8")
        # 2026-09-17: solo las lineas que cambio la feature; mutar el archivo entero medía los tests viejos (score 0.25)
        cambiadas = _lineas_cambiadas(f)
        muts = _mutants(orig, max_n=8, lineas=cambiadas) if _es_py(f) else _mutantes_texto(orig, max_n=8, lineas=cambiadas)
        por_archivo.append([(f, orig, m) for m in muts])
    mutantes: list[dict] = []   # ticket 09: por mutante, que tests fallaron (atribucion sobre las mismas corridas)
    for f, orig, m in (x for ronda in itertools.zip_longest(*por_archivo) for x in ronda if x):  # de a uno por archivo
        if tope is not None and total >= tope:
            break
        p = WT / f
        p.write_text(m, encoding="utf-8")
        try:
            if not _es_py(f) and compile_cmd and not _compila(_cmd("compile_cmd", [f]))[0]:
                continue  # mortinato: no compila, no cuenta
            total += 1
            muerto, fallaron = _accept_mata()
            killed += muerto
            mutantes.append({"archivo": f, "fallaron": fallaron})
        finally:
            p.write_text(orig, encoding="utf-8")
    if not total:
        state["mutation_score"] = None
        return rec_gate("mutacion", True, "sin mutantes posibles")
    score = round(killed / total, 3)
    state["mutation_score"] = score
    state["mutantes"] = mutantes
    vacuas = _propiedades_vacuas(mutantes)
    if minimo is not None and score < float(minimo):
        return rec_gate("mutacion", False, f"mutation score {score} < {minimo} ({killed}/{total})")
    if minimo is not None and vacuas:   # con mutation_min corren todos los mutantes: sin muertes, la propiedad es vacua
        return rec_gate("mutacion", False, f"propiedades vacuas (no matan ningun mutante): {vacuas}")
    return rec_gate("mutacion", True, f"mutation score {score} ({killed}/{total})" + ("" if minimo is not None else f" (observa: sin mutation_min, muestra de {tope})"))


def _propiedades_vacuas(mutantes: list[dict]) -> list[str]:
    """Ticket 09: una propiedad que no mata ningun mutante queda anotada como vacua (un ejemplo nunca)."""
    if any(m["fallaron"] is None for m in mutantes):
        state["atribucion"] = "sin atribucion por test: el runner de la aceptacion no da JUnit"
        return []
    props = [t for rel in TASK.accept_files if (WT / rel).is_file()
             for t in propiedades.propiedades(rel, (WT / rel).read_text(encoding="utf-8"))]
    state["propiedades_vacuas"] = sorted(t for t in props if not any(t in m["fallaron"] for m in mutantes))
    return state["propiedades_vacuas"]


def _en_base(fn):
    """Corre fn() con los archivos del plan en su version de HEAD (baseline) y despues los restaura."""
    keep = {f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()}
    # 2026-09-16: un archivo NUEVO del plan no existe en HEAD; `git checkout HEAD -- nuevo` explotaba y el job del server
    # terminaba en error sin detalle (macro-leadlag, export-mastery). En la base, el archivo nuevo simplemente no esta.
    en_head = [f for f in keep if _en_head(f)]
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


def _sin_aceptacion(fn):
    """Corre fn() sin los tests de aceptacion en el arbol. Son rojos en la base por diseño, y en un lenguaje compilado
    un test que no compila tumba la suite entera: el baseline salia rojo siempre y el gate de regresion solo observaba
    (ChatBot y Estudio, 2026-09-17)."""
    guardados = {f: (WT / f).read_bytes() for f in TASK.accept_files if (WT / f).exists()}
    for f in guardados:
        (WT / f).unlink()
    try:
        return fn()
    finally:
        for f, contenido in guardados.items():
            (WT / f).write_bytes(contenido)


def _suite_cmd_total(sha: str) -> tuple[bool, str]:
    """Otros lenguajes (`suite_cmd` en sdlc.toml): sin nombres de tests, compara verde/rojo contra el baseline."""
    # ponytail: exit code, no nombres; parsear la salida del runner (vitest, surefire) cuando un baseline rojo lo pida
    base_file = RUNS / f"suite-baseline-{sha}-cmd-sin-aceptacion.txt"
    if not base_file.exists():
        bok, blog = _en_base(lambda: _sin_aceptacion(lambda: sh(str(CFG["suite_cmd"]), timeout=CFG["suite_timeout_s"], keep=400000)))
        _write("docs/sdlc/suite-baseline.log", blog)
        if "TIMEOUT" in blog:
            return rec_gate("suite-total", False, f"baseline: {blog[:200]}")
        base_file.write_text("verde" if bok else "rojo", encoding="utf-8")
    ok, log = sh(_cmd_prop(str(CFG["suite_cmd"])), timeout=CFG["suite_timeout_s"], keep=400000)
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
    faltan = _desaparecidos() if state.get("area") else []
    if faltan:   # ticket 05: un fallo se ve en la suite; un test borrado no
        return rec_gate("suite-total", False, f"test desaparecido (fuera de los movimientos de la etapa 1): {faltan}")
    if CFG.get("suite_cmd"):
        return _suite_cmd_total(sha)
    base_file = RUNS / f"suite-baseline-{sha}.txt"
    if not base_file.exists():
        _, blog = _en_base(lambda: sh([PY, "-m", "pytest", *_suite_args("base"), *_prop_args()], timeout=CFG["suite_timeout_s"], keep=400000))
        _write("docs/sdlc/suite-baseline.log", blog)
        if "TIMEOUT" in blog:  # D15: un baseline vacio por timeout se cacheaba y volvia "nuevos" a los rojos previos
            return rec_gate("suite-total", False, f"baseline: {blog[:200]}")
        base_file.write_text("\n".join(re.findall(r"(?m)^(?:FAILED|ERROR) \S+", blog)) + "\n", encoding="utf-8")
    # D8: la suite del worktree traia 594 ERROR (setup) que el gate no miraba porque solo leia FAILED.
    base = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", base_file.read_text(encoding="utf-8")))
    ok, log = sh([PY, "-m", "pytest", *_suite_args("final"), *_prop_args()], timeout=CFG["suite_timeout_s"], keep=400000)
    _write("docs/sdlc/suite.log", log)
    now = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", log))
    new = sorted(now - base)
    if new and "TIMEOUT" not in log:
        # D2 r1: 3 tests de providers fallaron en la suite y pasaron solos -> flaky bajo carga.
        # Un fallo nuevo cuenta como regresion solo si se repite AISLADO (determinista, sin juicio).
        _, rlog = sh([PY, "-m", "pytest", *new[:50], "-q", "-rfE", "-p", "no:cacheprovider", *_prop_args()], keep=400000)
        flaky = sorted(set(new) - set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", rlog)))
        new = sorted(set(new) - set(flaky))
        state["suite_flaky"] = flaky
    state["suite_total"] = test_counts(log) | {"new_failures": new, "baseline_failures": len(base)} if test_counts(log) else {"log": log[-300:]}
    if "TIMEOUT" in log:
        return rec_gate("suite-total", False, log[:200])
    return rec_gate("suite-total", not new, "sin fallos nuevos" if not new else f"fallos nuevos: {new}")


def _desaparecidos() -> list[str]:
    """Archivos de test de HEAD que ya no estan en el worktree y no figuran en los movimientos de la etapa 1."""
    out = subprocess.run(["git", "diff", "--name-only", "--diff-filter=D", "HEAD", "--", "tests/"], cwd=WT,
                         capture_output=True, text=True, encoding="utf-8").stdout.split()
    movidos = {m["de"] for m in state.get("movimientos", [])}
    return [f for f in out if f not in movidos]


def _accept_paths():
    paths = list(TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        paths.append(REVIEW_REL)
    return paths


def _accept_por_cmd() -> str | None:
    """El comando del repo es el oraculo si no hay tests nombrados o si estan en otro lenguaje; None = pytest."""
    cmd = ACCEPT_CMD or CFG.get("accept_cmd")  # payload o sdlc.toml
    return str(cmd) if cmd and (not TASK.accept_files or not all(_es_py(f) for f in TASK.accept_files)) else None


def _accept_desc() -> str:
    """La aceptacion como la corre accept(), para los prompts (decia pytest tambien en repos TypeScript)."""
    return _accept_por_cmd() or f"pytest {' '.join(_accept_paths())} -q"


def accept():
    cmd = _accept_por_cmd()
    if cmd:
        ok, log = sh(_cmd_prop(str(cmd)))  # sin tests nombrados, o tests en otro lenguaje: el comando del repo es el oraculo
        if (WT / REVIEW_REL).exists():
            # 2026-09-17: la revision deja un test pytest; en ChatBot (Java) `mvn test` no lo corria y un BLOCK real
            # (clave de deduplicacion con "|" sin escapar) paso como "sin evidencia". Corre despues: puede usar lo compilado.
            rok, rlog = sh([PY, "-m", "pytest", REVIEW_REL, "-q", "-p", "no:cacheprovider"])
            ok, log = ok and rok, f"{log}\n{rlog}"
        return ok, log
    return sh([PY, "-m", "pytest", *_accept_paths(), "-q", "-p", "no:cacheprovider", *_prop_args()])


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
    """fnmatch con `**/` opcional: `pkg/**/*.py` incluye `pkg/a.py`. Techo y areas comparten esta semantica."""
    return areas.calza(f, pat)


def _en_techo(f: str) -> bool:
    t = _techo()
    return f in _need_files() or not t or any(_calza(f, pat) for pat in t)  # sin techo (bench, FEATURES viejas): todo vale


def _tamano(files: list[str]) -> dict:
    """orchestration-e4d: archivos del plan, tokens (bytes/4) de los que ya existen y acoplamiento: modulos FUERA del
    plan que comparten literales de datos con los archivos del plan (mmorch.impacto.lectores).
    Escalones x4 del ticket: minima <=4k, chica <=16k, media <=64k, grande >64k."""
    from mmorch.impacto import lectores
    tok = sum((WT / f).stat().st_size // 4 for f in files if (WT / f).is_file())
    escalon = "minima" if tok <= 4000 else "chica" if tok <= 16000 else "media" if tok <= 64000 else "grande"
    lect: set[str] = set()
    for f in files:
        if (WT / f).is_file():
            lect.update(lectores(WT, WT / f))
    fuera = sorted(lect - set(files))
    return {"archivos": len(files), "tokens": tok, "escalon": escalon, "acople": len(fuera), "lectores": fuera[:20]}


def _informe(f: str, tope: int = 4000) -> str:
    """Informe de impacto de un archivo que ya existe en el worktree (lectores acoplados), recortado."""
    from mmorch.impacto import report
    try:
        return report(WT, WT / f)[:tope] if (WT / f).is_file() else ""
    except Exception:  # ponytail: el informe ayuda, nunca rompe la etapa
        return ""


def _informe_spec(spec_md: str) -> str:
    """orchestration-e4d: informe de impacto de los archivos que la spec nombra y ya existen. El reasoner planea sin
    ver el repo; asi sabe que otros modulos leen lo que va a cambiar (banco del mapa de impacto: 42% -> 96%)."""
    nombrados = [f for f in dict.fromkeys(re.findall(r"[\w./-]+\.(?:" + "|".join(_exts()) + r")\b", spec_md))
                 if (WT / f).is_file()]
    return "\n\n".join(x for x in (_informe(f, 3000) for f in nombrados[:6]) if x)[:8000]


def _proponer_division(spec_md: str, plan_md: str) -> None:
    """Plan grande: el planner propone dividir la tarea y la propuesta queda en docs/sdlc/division.md. La etapa igual
    se detiene: cada parte es una feature nueva con su test de aceptacion, y eso lo decide un humano o Claude."""
    max_a = int(CFG.get("tamano_max_archivos", 8))
    try:
        out = _planner("Sos un tech lead. Dividis una tarea grande en features chicas e independientes.",
                       f"Este plan supera el tamano que el pipeline maneja bien ({state['plan_tamano']}). Dividi la "
                       f"tarea en 2 a 4 features, cada una con a lo sumo {max_a} archivos, en el orden en que conviene "
                       "construirlas. Para cada feature: titulo, requisitos R<n> de la spec que cubre, archivos y un "
                       "criterio de aceptacion verificable. Formato: '## Feature N: titulo'.\n\n"
                       f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}")
    except RuntimeError as e:  # el planner caido no tapa el motivo del corte
        out = f"(no se pudo proponer la division: {e})"
    _write("docs/sdlc/division.md", out)
    write_supervision("ESCALATE_HUMAN: tarea grande; propuesta de division en docs/sdlc/division.md")


def gate_tamano(files: list[str]) -> tuple[bool, str]:
    """Baranda de tamaño (orchestration-e4d, 2026-10-04). Las 107 corridas medidas tienen <= 8 archivos y <= 16k tokens;
    fuera de ese rango no hay datos, asi que la etapa 3 frena y pide dividir la tarea. El exito sin Claude cae con los
    archivos (1: 0.89, 2-3: 0.81, 4-7: 0.65) pero no hay evidencia de que dividir ayude: los umbrales se revisan con
    20 corridas de 4 o mas archivos. Por repo: `tamano_max_archivos` y `tamano_max_tokens` en sdlc.toml."""
    t = state["plan_tamano"] = _tamano(files)
    max_a, max_t = int(CFG.get("tamano_max_archivos", 8)), int(CFG.get("tamano_max_tokens", 16000))
    ok = t["archivos"] <= max_a and t["tokens"] <= max_t
    detalle = f"{t['archivos']} archivos, {t['tokens']} tokens ({t['escalon']})"
    if not ok:
        detalle += f": supera {max_a} archivos o {max_t} tokens; dividi la tarea en features mas chicas"
    return rec_gate("tamano-plan", ok, detalle)


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
    if state.get("area"):   # ticket 01: el plan no sale del area (los archivos sin area valen)
        ar = areas.areas(CFG)
        otra_area = {f: otras for f in files if (otras := [a for a in areas.areas_de(f, ar) if a != state["area"]])}
        for f, otras in otra_area.items():
            for a in otras:
                _fuera_de_area(a, f)
        if otra_area:
            return rec_gate("plan-allowlist", False, f"plan lista archivos de otra area (anotados en bd): {otra_area}")
    return rec_gate("plan-allowlist", True, f"{files}")


def _bd(args: list[str], cwd) -> tuple[bool, str]:
    """`bd` en el host. Devuelve (ok, stdout) o (False, motivo)."""
    try:
        r = subprocess.run(["bd", *args], cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
    except (OSError, subprocess.TimeoutExpired) as e:
        return False, str(e)
    return r.returncode == 0, r.stdout if r.returncode == 0 else r.stderr[-300:]


def _fuera_de_area(area: str, f: str) -> None:
    """Ticket 01: un archivo que el plan quiso tocar en otra area va a bd en el momento. Label fija por area: con un
    issue abierto suma una nota, sin issue lo crea. bd del repo destino si tiene .beads, si no el de orchestration.
    Copia en division.md y en el run-log (`fuera_de_area`). Un bd que falla no rompe la corrida."""
    vistos = state.setdefault("fuera_de_area", {}).setdefault(area, [])
    if f in vistos:
        return
    vistos.append(f)
    div = WT / "docs/sdlc/division.md"
    previo = div.read_text(encoding="utf-8") if div.exists() else "# Division por areas\n"
    if "## Fuera de area (plan)" not in previo:
        previo = previo.rstrip("\n") + "\n\n## Fuera de area (plan)\n\n"
    _write("docs/sdlc/division.md", previo + f"- `{f}` (area {area}): tarea {TASK_NAME}\n")
    label, nota = f"sdlc-fuera-de-area:{area}", f"{f} (tarea {TASK_NAME}, desde el area {state.get('area')})"
    cwd = FEAT["repo"] if FEAT and (pathlib.Path(FEAT["repo"]) / ".beads").is_dir() else str(_ORCH)
    ok, out = _bd(["list", "--label", label, "--status", "open", "--json"], cwd)
    try:
        abiertos = json.loads(out) if ok else None
    except ValueError:
        abiertos = None
    if abiertos:
        ok, out = _bd(["update", abiertos[0]["id"], "--append-notes", nota], cwd)
    elif abiertos is not None:
        ok, out = _bd(["create", "--title", f"sdlc: cambios pendientes en el area {area}", "--labels", label,
                       "--description", f"Archivos que un plan del SDLC quiso tocar fuera de su area:\n{nota}", "--json"], cwd)
    rec_gate("bd-fuera-de-area", ok, f"{f} -> {label}" if ok else f"bd fallo ({out[:200]}): {f} queda en division.md")


def _resolver_area() -> str | None:
    """Ticket 01: el area sale de los `files` del llamador (sin `files`, de las rutas del texto) cruzados con `[areas]`
    de sdlc.toml. Devuelve la ruta de su spec, o None (sin area: el camino de hoy). Dos areas cortan antes de la etapa 1
    con un division.md determinista, sin LLM."""
    for k in ("area", "area_spec", "area_archivos"):
        state.pop(k, None)
    ar = areas.areas(CFG)
    files = list((FEAT or {}).get("files") or [])
    por = areas.area_de_tarea(files, TASK.task, ar, _exts()) if ar else {}
    if not por:
        return None
    tests_area = [f for f in files if f.startswith("tests/sdlc/")]
    if tests_area:
        raise StageFailed("0-areas", f"files bajo tests/sdlc/ en modo area: {tests_area} (esos tests los escribe la etapa 1)")
    state["areas_solapadas"] = areas.solapados(WT, ar)
    if len(por) > 1:
        _write("docs/sdlc/division.md", areas.division(por))
        write_supervision("ESCALATE_HUMAN: la tarea toca varias areas; division en docs/sdlc/division.md")
        rec_gate("areas", False, f"{list(por)}")
        raise StageFailed("0-areas", f"la tarea toca {len(por)} areas {list(por)}: division en docs/sdlc/division.md")
    ((area, archivos),) = por.items()
    state.update(area=area, area_spec=areas.ruta_spec(area, CFG), area_archivos=archivos)
    rec_gate("areas", True, f"area {area}: {archivos}")
    return state["area_spec"]


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
    """Trazabilidad lado plan: cada R<n> de la spec aparece en el plan; cada archivo cita >= 1 R<n>. Un item es una
    linea de lista o una fila de tabla; un rango `R2–R10` cita R2..R10 (SSB F7a)."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    plan_block = re.sub(r"\bR(\d+)\s*[–-]\s*R(\d+)\b",
                        lambda m: ", ".join(f"R{i}" for i in range(int(m[1]), int(m[2]) + 1)), plan_block)
    huerfanos = sorted(i for i in ids if not re.search(rf"\b{i}\b", plan_block))
    items = re.findall(r"(?m)^\s*[-*]\s*(.*)$", plan_block) + _filas_tabla(plan_block)

    def nombra(it: str, f: str) -> bool:  # la ruta, o su nombre suelto si el plan lo escribio sin carpeta (F4b)
        return f in it or bool(re.search(r"(?<![\w/.-])" + re.escape(f.rsplit("/", 1)[-1]) + r"(?![\w.-])", it))
    sin_id = [f for f in files if not any(nombra(it, f) and re.search(r"\bR\d+\b", it) for it in items)]
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
        ok, log, previos = _compila(_cmd("compile_cmd", otros))
        return rec_gate("G3-compile", ok, _nota_compila(previos) if ok else log[-800:])
    return rec_gate("G3-compile", True, "compila")


_COMPILE_BASE: dict[str, set[str]] = {}  # comando -> errores de la base sin la aceptacion; se vacia en configure()


def _errores_compilador(log: str) -> set[str]:
    """Errores sin linea ni columna: `ruta(l,c): error ...` (tsc) o `ruta:l:c: ...` (javac, go, gcc). Sin warnings."""
    return {f"{m[1]}: {m[2].strip()}"
            for m in re.finditer(r"(?m)^\s*(\S+?\.\w+)(?:\(\d+,\d+\)|:\d+(?::\d+)?):?\s+(.+)$", log)
            if not m[2].lower().startswith("warning")}


def _compila(cmd: str) -> tuple[bool, str, list[str]]:
    """Corre `cmd` y descuenta los errores que la base ya tenia SIN la aceptacion de esta feature. Un compilador de
    proyecto entero (tsc) ve los tests rojos por diseño de OTRAS features: en SSB F3a el TS2307 del test de F3b tumbaba
    G3. Una regresion en otro archivo sigue bloqueando (su error no esta en la base), igual que un error del test de
    aceptacion propio. Sin errores legibles en la salida no descuenta nada. Devuelve (ok, log, errores previos)."""
    ok, log = sh(cmd)
    ahora = set() if ok else _errores_compilador(log)
    if not ahora or not state.get("plan_files"):
        return ok, log, []
    if cmd not in _COMPILE_BASE:
        _, blog = _en_base(lambda: _sin_aceptacion(lambda: sh(cmd)))
        _COMPILE_BASE[cmd] = _errores_compilador(blog)
    previos = sorted(ahora & _COMPILE_BASE[cmd])
    if len(previos) < len(ahora):
        return False, log, previos
    state["compile_previos"] = previos
    return True, log, previos


def _nota_compila(previos: list[str]) -> str:
    if not previos:
        return "compila"
    return f"compila salvo {len(previos)} errores previos de la base, ajenos a esta feature: {'; '.join(previos)[:600]}"


def gate_test_compile() -> tuple[bool, str]:
    """Equivalente a mvn test-compile: los tests importan y se recolectan."""
    if not _accept_paths():
        return rec_gate("test-compile", True, "sin tests nombrados (accept_cmd)")
    if not all(_es_py(f) for f in _accept_paths()):
        cmd = CFG.get("compile_cmd")
        if not cmd:
            return rec_gate("test-compile", True, "tests no-Python sin compile_cmd: no se compila")
        ok, log, previos = _compila(_cmd("compile_cmd", _accept_paths()))
        return rec_gate("test-compile", ok, ("ok" if not previos else _nota_compila(previos)) if ok else log[-800:])
    ok, log = sh([PY, "-m", "pytest", *_accept_paths(), "--collect-only", "-q", "-p", "no:cacheprovider"])
    return rec_gate("test-compile", ok, "ok" if ok else log[-800:])


def ledger_usd() -> float | None:
    from mmorch.metrics import log_path, read_events
    if not log_path().exists():
        return None
    t0 = state.get("t0_epoch") or 0
    usd = 0.0
    state["usd_by_family"] = {}
    for r in read_events()[-8000:]:
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
    """Los tests para los prompts por API, sin el codigo de las propiedades (ticket 08: el coder ve su salida)."""
    t = "\n\n".join(f"# {k}\n{propiedades.sin_propiedades(k, (WT / k).read_text(encoding='utf-8'))}" for k in TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        t += f"\n\n# {REVIEW_REL} (revision de Claude, no se toca)\n" + (WT / REVIEW_REL).read_text(encoding="utf-8")
    return t


def write_supervision(note: str):
    p = WT / "docs" / "sdlc" / "supervision.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    prev = p.read_text(encoding="utf-8") if p.exists() else "# supervision.md — driver_py\n\n"
    p.write_text(prev + f"\n## candidato {time.strftime('%H:%M:%S')}\n{note}\n", encoding="utf-8")


def _revert(backup):
    for f, old in backup.items():
        _write(f, old)


def _dump_diag_case(log: str, kind: str = "test") -> None:
    d = RUNS / "diag-cases"; d.mkdir(parents=True, exist_ok=True)
    case = {"task": TASK_NAME, "phase": PHASE, "kind": kind, "base_sha": state.get("base_sha"), "log": log[-20000:],
            "files": _all_code(), "orig": dict(_orig_files), "tests": _tests_text(), "task_text": TASK.task,
            "compile_cmd": CFG.get("compile_cmd")}
    stem = PHASE if kind == "test" else f"{PHASE}-{kind}"
    (d / f"{stem}-{int(time.time())}.json").write_text(json.dumps(case, ensure_ascii=False), encoding="utf-8")


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
            out = llm(CODER, _rol(f) + "Devolves SOLO el archivo entero.",
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


def _revisor(prompt: str, *, mode: str = "edit", timeout: float | None = None) -> dict:
    """Ticket 11: el agente que revisa y corrige es configurable por repo (`reviewer_cmd` en sdlc.toml, prompt por
    stdin); sin esa clave manda la seam de mmorch (`claude -p` por default, env MMORCH_EXECUTOR)."""
    from mmorch.claude_exec import CmdExecutor, get_executor
    os.environ.pop("CLAUDECODE", None)  # ponytail: el CLI no anida dentro de una sesion Claude Code
    cmd = CFG.get("reviewer_cmd")
    ex = CmdExecutor(str(cmd)) if cmd else get_executor()
    r = ex.run(prompt, str(WT), mode=mode, timeout=float(timeout or CFG["cmd_timeout_s"]))
    return {"ok": r.ok, "result": r.result, "returncode": r.returncode, "steps": r.steps}


def claude_fix(log) -> tuple[bool, str]:
    """Nivel 3: Claude en el worktree (modo edit). Deja correccion; el test de aceptacion decide."""
    state["escalated_to_claude"] = True
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(f"Los tests de aceptacion fallan. Arregla el codigo en {state['plan_files']} hasta que "
                 f"`{_accept_desc()}` pase. NO toques tests ni docs. Al final responde en una linea que cambiaste.\n\n"
                 f"TAREA:\n{TASK.task}\n\nSALIDA:\n{log}")
    write_supervision(f"nivel 3 Claude (rc={r.get('returncode')}): {(r.get('result') or '')[:2000]}")
    gate_alcance("claude-fix-alcance", before, tuple(state["plan_files"]))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    ok, log = accept()
    if ok:
        state["suite_total"] = test_counts(log)
    return ok, "Claude verde" if ok else "Claude rojo: escala a humano"


# Archivos de entorno que Claude puede tocar para dejar andar el compilador (sin tocar el codigo ni los tests).
_ENV_FILES = ("sdlc.toml", "package.json", "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "tsconfig",
              "node_modules/", "pom.xml", "build.gradle", "settings.gradle", "gradle", "mvnw", ".mvn/",
              "requirements", "pyproject.toml")


def _claude_compile(log: str, compiles, *, env: bool) -> tuple[bool, str]:
    """Claude en el worktree para la etapa build (orchestration-7ys). env=True: falta el compilador; resuelve el
    entorno del proyecto sin tocar el sistema. env=False: errores de compilacion; arregla los archivos del plan.
    Exito = compila (no los tests de aceptacion, que corren en la etapa 5)."""
    state["escalated_to_claude"] = True
    state["claude_calls"] += 1
    before = _tree()
    cmd = CFG.get("compile_cmd") or "py_compile"
    if env:
        prompt = (f"El comando de compilacion `{cmd}` no encuentra su herramienta o una dependencia del entorno "
                  f"(no es un error del codigo).\n\nSALIDA:\n{log}\n\n"
                  "Resolve el entorno SIN instalar nada global ni fuera de este directorio: usa la herramienta local del "
                  "proyecto (npx, mvnw, gradlew, el venv del repo), agrega la dependencia de desarrollo al manifiesto e "
                  "instalala en el proyecto, o corregi `compile_cmd` en sdlc.toml. No toques codigo fuente, tests ni docs. "
                  "Si no se puede sin tocar el sistema, no cambies nada. Al final responde en una linea que hiciste o que falta.")
        if CFG.get("contenedor"):  # ticket 07 de builder-e2e: sin este aviso Claude intentaba `npm install` sin red
            prompt += (f"\n\nLos comandos corren en la imagen `{CFG['contenedor']}` SIN RED: no instales nada. Si alcanza con "
                       "cambiar la configuracion del proyecto (tsconfig, sdlc.toml), hacelo. Si la imagen necesita algo, no "
                       "cambies nada y nombra que hay que agregar a la imagen.")
        allowed = _ENV_FILES
    else:
        prompt = (f"El codigo no compila con `{cmd}`. Arregla {state['plan_files']} hasta que compile. "
                  f"NO toques tests ni docs. Al final responde en una linea que cambiaste.\n\nTAREA:\n{TASK.task}\n\nSALIDA:\n{log}")
        allowed = tuple(state["plan_files"])
    r = _revisor(prompt)
    write_supervision(f"nivel 3 Claude build ({'entorno' if env else 'compila'}, rc={r.get('returncode')}): "
                      f"{(r.get('result') or '')[:2000]}")
    gate_alcance("claude-build-alcance", before, allowed)
    if env:
        new = _toml(WT).get("compile_cmd")
        if new:
            CFG["compile_cmd"] = new
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    return compiles()


def _compile_ladder(log: str, compiles) -> tuple[bool, str, str]:
    """Escalera de la etapa build (orchestration-7ys): diagnostico x2 y despues Claude; exito = compila.
    Antes la etapa cortaba tras 3 vueltas del coder sin escalar. Cada entrada queda como caso replayable."""
    _dump_diag_case(log, kind="compile")
    for i in range(REASONER_TRIES):
        written = _all_code()
        pick = llm(DIAG, 'Sos un diagnosticador. Respondes SOLO JSON: {"files": [paths], "instructions": {path: "una instruccion"}}. Una instruccion por archivo.',
                   f"El codigo no compila. Diagnostica y indica QUE cambiar.\n\nERROR DEL COMPILADOR:\n{log}\n\n"
                   f"TAREA:\n{TASK.task}\n\nARCHIVOS:\n{_joined(written)}")
        write_supervision(f"reasoner build {i+1}: {pick[:2000]}")
        try:
            data = json.loads(strip_fence(pick))
            files = [f for f in data.get("files", []) if f in written]
            instr = data.get("instructions") or {}
        except Exception:
            files, instr = [], {}
        files = files or list(written)[:1]
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, _rol(f) + "Devolves SOLO el archivo entero.",
                      f"Aplica ESTA instruccion a {f} para que compile. No toques los tests.\n"
                      f"INSTRUCCION: {instr.get(f, 'corregi el error del compilador')}\n\n"
                      f"ERROR DEL COMPILADOR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code)
        if not gate_baseline()[0]:
            _revert(backup)
            continue
        ok, log = compiles()
        state["compile_ladder"] = state.get("compile_ladder", []) + [{"i": i + 1, "files": files, "ok": ok}]
        if ok:
            return True, f"reasoner build {i+1} compila", log
    ok, log = _claude_compile(log, compiles, env=False)
    return ok, "Claude build compila" if ok else "Claude build rojo: escala a humano", log


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
            or one_file("class A: pass", "a.py") != "class A: pass" \
            or one_file("public class Negocio {}", "x/Opcion.java") != "" \
            or one_file("public class Opcion {\n    public static class Item {}\n}", "x/Opcion.java") == "":
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


def _slug() -> str:
    return re.sub(r'[^a-z0-9]+', '_', TASK_NAME.lower()).strip('_')[:40]


def _archivar_spec() -> None:
    """orchestration-3s7: spec y plan quedan en docs/specs/<feature>/ (spec-anchored). docs/sdlc/ es la mesa de
    trabajo que la feature siguiente pisa. La ficha de la feature, si existe, va al lado."""
    for n in ("spec.md", "plan.md"):
        src = WT / "docs/sdlc" / n
        if src.exists():
            _write(f"docs/specs/{_slug() or 'sdlc'}/{n}", src.read_text(encoding="utf-8"))
    ficha = _ficha()
    if ficha:
        _write(f"docs/specs/{_slug() or 'sdlc'}/ficha.md", ficha.read_text(encoding="utf-8"))


def _ficha() -> pathlib.Path | None:
    """Ficha de la feature en docs/sdlc/features/ (orchestration-4u6): el nombre se compara pasado por la regla de
    `_slug()`, porque las fichas reales usan guiones y mayusculas (`F1-escena.md`)."""
    d = WT / "docs/sdlc/features"
    norma = lambda s: re.sub(r"[^a-z0-9]+", "_", s.lower()).strip("_")[:40]  # noqa: E731
    return next((p for p in sorted(d.glob("*.md")) if norma(p.stem) == _slug()), None) if d.is_dir() else None


def _spec_area_md() -> str:
    """La spec viva del area en el worktree; un area sin spec parte de un esqueleto (ticket 03)."""
    p = WT / state["area_spec"]
    if p.is_file():
        return p.read_text(encoding="utf-8")
    return f"# Spec del area {state['area']}\n\n## Contrato\n\n## Requisitos\n\n## Trazabilidad\n"


def _ids_del_area() -> set[int]:
    """Numeros usados en el area: requisitos de la spec y archivos de test `tests/sdlc/<area>/test_R<n>_*`."""
    ids = set(areas.requisitos(_spec_area_md())) | set(areas.archivos_por_id(WT, state["area"]))
    return {areas.numero(i) for i in ids}


def _ids_con_prop(rels) -> list[str]:
    return sorted({i for rel in rels if (WT / rel).is_file()
                   for i in propiedades.gate_propiedades(rel, (WT / rel).read_text(encoding="utf-8"))[1]}, key=areas.numero)


def _archivar_area() -> tuple[bool, str]:
    """Ticket 06: la etapa 6 funde el delta en la spec viva por codigo, deriva Contrato y Trazabilidad, y archiva delta,
    plan y ficha en changes/<slug>-<sha>. El delta archivado marca la fusion hecha: una etapa 6 reanudada no la repite."""
    area, spec_rel = state["area"], state["area_spec"]
    base = f"docs/specs/{area}/changes/{_slug() or 'sdlc'}-{_sha_corrida()}"
    if not (WT / f"{base}.md").exists():
        delta = (WT / "docs/sdlc/delta.md").read_text(encoding="utf-8")
        try:
            fus = areas.fusionar(_spec_area_md(), delta)
        except ValueError as e:
            return rec_gate("fusion", False, str(e))
        _write(f"{base}.plan.md", (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8"))   # el Contrato lee los planes
        viva = areas.regenerar(fus, WT, area)
        errores = areas.gate_traza_area(viva, WT)
        if errores:
            (WT / f"{base}.plan.md").unlink()
            return rec_gate("fusion", False, f"requisito fusionado sin archivo de test: {errores}")
        _write(spec_rel, viva)
        ficha = _ficha()
        if ficha:
            _write(f"{base}.ficha.md", ficha.read_text(encoding="utf-8"))
        _write(f"{base}.md", delta)   # ultimo: su existencia dice que la fusion termino
    # nivel 6.2 del curso: el PR se lee desde el diff de la spec, en su propio commit
    _commit(f"sdlc: {TASK_NAME} - spec del area {area} (delta, fusion y removidos)", [f"docs/specs/{area}", spec_rel])
    return rec_gate("fusion", True, f"{spec_rel} + {base}.md")


def _commit(msg: str, rutas: list[str] | None = None) -> bool:
    """git add y commit de todo el arbol o solo de `rutas`; sin cambios no commitea."""
    lim = ["--", *dict.fromkeys(rutas)] if rutas else []
    subprocess.run(["git", "add", "-A", *lim], cwd=WT, check=True)
    if subprocess.run(["git", "diff", "--cached", "--quiet", *lim], cwd=WT).returncode == 0:
        return False
    subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m", msg, *lim],
                   cwd=WT, check=True)
    return True


def _pide_propiedades(cuando: str = "La propiedad puede pasar hoy si su ID tiene otro test que falla.") -> str:
    """Pedido de tests de propiedades para la etapa 1 (o la 0); vacio sin libreria (ticket 10: sin propiedades)."""
    lib = (state.get("propiedades") or {}).get("libreria")
    if not lib:
        return ""
    como = "Hypothesis (`@given`)" if lib == "hypothesis" else "fast-check (`fc.assert(fc.property(...))`)"
    return (f" Si un requisito es una regla que vale para un conjunto de entradas generable, sumale un test de propiedades "
            f"con {como}, nombrado test_R<n>_prop_..., en el archivo de ese ID. {cuando}")


def _marco(rel: str) -> str:
    return "pytest" if _es_py(rel) else f"framework de tests del repo; lo corre `{ACCEPT_CMD or CFG.get('accept_cmd')}`"


def _lista_tests(rels) -> str:
    """Archivos y nombres de test, para la parada unica (las propiedades se ven junto con los demas)."""
    return "\n".join(f"- {r}: {', '.join(dict.fromkeys(areas.tests_en((WT / r).read_text(encoding='utf-8'), r)))}"
                     for r in rels if (WT / r).is_file())


def _parada(rels: list[str], extra: str = "") -> tuple[bool, str]:
    """Una sola parada humana (approve_accept) con todos los oraculos nuevos de las etapas 0 y 1."""
    state["accept_files"] = list(TASK.accept_files)
    _seed_accept()
    snapshot_baseline()
    if CFG.get("approve_accept", True):
        state["awaiting_approval"] = rels[0]
        write_supervision(f"ESPERA_HUMANO: aprobar {', '.join(rels)} y reanudar desde la etapa 2 (from_stage=2)\n"
                          f"{_lista_tests(list(TASK.accept_files))}{extra}")
        return False, f"esperando aprobacion humana de {', '.join(rels)} (reanudar desde la etapa 2)"
    return True, f"{rels}: sin aprobacion (approve_accept=false)"


def _gate_archivos_por_id(rels: list[str], permitidos: list[str]) -> list[str]:
    """Modo area (tickets 04 y 05): `tests/sdlc/<area>/test_R<n>_<slug>.<ext>`, un ID por archivo y por test, con un
    numero permitido; el nombre base no se repite en tests/ (pytest no importa dos modulos con el mismo nombre)."""
    errores, d = [], f"tests/sdlc/{state['area']}/"
    todos = subprocess.run(["git", "ls-files", "-co", "--exclude-standard", "tests"], cwd=WT, capture_output=True,
                           text=True).stdout.split()
    for rel in rels:
        m = re.fullmatch(re.escape(d) + r"test_(R\d+)_\w+\.\w+", rel)
        if not m:
            errores.append(f"{rel}: el nombre debe ser {d}test_R<n>_<slug>.<ext>")
            continue
        rid, nombres = m.group(1), areas.tests_en((WT / rel).read_text(encoding="utf-8"), rel)
        ajenos = [t for t in nombres if not t.startswith(f"test_{rid}_")]
        if not nombres or ajenos:
            errores.append(f"{rel}: cada test cubre solo {rid} (test_{rid}_...); ajenos o faltantes: {ajenos}")
        if rid not in permitidos:
            errores.append(f"{rel}: {rid} esta fuera de los numeros ofrecidos {permitidos}")
        if any(o != rel and o.rsplit("/", 1)[-1] == rel.rsplit("/", 1)[-1] for o in todos):
            errores.append(f"{rel}: otro archivo de tests/ tiene el mismo nombre")
        errores += propiedades.gate_propiedades(rel, (WT / rel).read_text(encoding="utf-8"))[0]
    return errores


def _nuevos_del_area(gid: str, before: dict[str, str], extra: tuple[str, ...] = ()) -> tuple[list[str] | None, str]:
    """Archivos que Claude creo o cambio en tests/sdlc/<area>/. Tocar otra cosa (salvo `extra`) revierte y rechaza."""
    d = f"tests/sdlc/{state['area']}/"
    ok, nota = gate_alcance(gid, before, (d, *extra))
    if not ok:
        return None, nota
    after = _tree()
    return sorted(p for p in after if p.startswith(d) and before.get(p) != after[p]), "ok"


def _fallan(rels: list[str], res: dict[str, bool] | None, log: str) -> list[str]:
    """Tests de `rels` que fallaron segun JUnit (o, sin JUnit, los nombres que aparecen como FAILED en el log)."""
    nombres = [t for r in rels for t in areas.tests_en((WT / r).read_text(encoding="utf-8"), r)]
    if res is not None:
        return [t for t in nombres if res.get(t) is False]
    return [t for t in nombres if re.search(rf"\b{t}\b", log) and "FAIL" in log]


@stage("0-caracterizar")
def caracterizar():
    """Ticket 03: los archivos de la tarea que la spec del area no nombra en Contrato se caracterizan antes del delta.
    Claude escribe un archivo por requisito base (`tests/sdlc/<area>/test_R<n>_<slug>`, menores numeros libres). Gates
    deterministas: solo esos archivos nuevos, pasan en HEAD, matan >= 1 mutante del archivo caracterizado
    (`caracterizacion_min` opcional). La parada humana es la de la etapa 1: una sola para los dos."""
    if not state.get("area"):
        return True, "sin area: no caracteriza"
    cubiertas = set(areas.rutas_cubiertas(_spec_area_md()))
    nuevos = [f for f in state.get("area_archivos", []) if f not in cubiertas and (WT / f).is_file()]
    if not nuevos:
        return True, "sin archivos nuevos en el area"
    area, ext = state["area"], _exts()[0]
    ofrecidos = [f"R{n}" for n in areas.libres(_ids_del_area(), 10)]
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(f"Escribi tests de CARACTERIZACION ({_marco(f'x.{ext}')}, sin red) del comportamiento ACTUAL de "
                 f"{nuevos}: fijan lo que el codigo hace hoy, no lo que la tarea pide. Un archivo por requisito: "
                 f"tests/sdlc/{area}/test_R<n>_{_slug()}.{ext}, con tests test_R<n>_... del mismo ID. Usa los IDs en este "
                 f"orden, sin saltear: {', '.join(ofrecidos)}. Todos los tests DEBEN pasar hoy y detectar un cambio del "
                 f"codigo." + _pide_propiedades("Tambien debe pasar hoy.") + " No toques ningun otro archivo."
                 f"\n\nTAREA (contexto):\n{TASK.task}")
    write_supervision(f"caracterizacion (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1000]}")
    tests, nota = _nuevos_del_area("caracterizacion-alcance", before)
    if tests is None:
        return False, nota
    tocados = [t for t in tests if t in before]
    if tocados or not tests:
        gate_alcance("caracterizacion-alcance", before, ())   # revierte lo tocado
        return rec_gate("caracterizacion-alcance", False, f"la etapa 0 solo crea archivos nuevos; toco {tocados}" if tocados
                        else "Claude no dejo tests de caracterizacion")
    ids = sorted({re.match(r"test_(R\d+)_", t.rsplit("/", 1)[-1]).group(1) for t in tests
                  if re.match(r"test_(R\d+)_", t.rsplit("/", 1)[-1])}, key=areas.numero)
    errores = _gate_archivos_por_id(tests, ofrecidos)
    if not errores and ids != ofrecidos[:len(ids)]:
        errores.append(f"los requisitos base toman los menores numeros libres: {ofrecidos[:len(ids)]} (dejo {ids})")
    if errores:
        return rec_gate("caracterizacion-ids", False, "; ".join(errores))
    ok, log, res = _corre_con_junit(tests)
    if not ok:
        return rec_gate("caracterizacion-verde", False, f"falla en HEAD: {_fallan(tests, res, log) or log[-400:]}")
    from mmorch.checkers import _mutants
    mutantes = []
    for f in nuevos:   # solo el archivo caracterizado, todas sus lineas (no hay cambio todavia)
        orig = (WT / f).read_text(encoding="utf-8")
        for m in (_mutants(orig, max_n=8) if _es_py(f) else _mutantes_texto(orig, max_n=8)):
            (WT / f).write_text(m, encoding="utf-8")
            try:
                mutantes.append({"archivo": f, "muerto": not _corre_con_junit(tests, mut=True)[0]})
            finally:
                (WT / f).write_text(orig, encoding="utf-8")
    muertos = sum(m["muerto"] for m in mutantes)
    score = round(muertos / len(mutantes), 3) if mutantes else None
    state["caracterizacion"] = {"archivos": nuevos, "tests": tests, "ids": ids, "score": score, "mutantes": mutantes}
    minimo = CFG.get("caracterizacion_min")
    if mutantes and not muertos:
        return rec_gate("caracterizacion-mutantes", False, f"los tests no matan ningun mutante de {nuevos} (vacuos)")
    if score is not None and minimo is not None and score < float(minimo):
        return rec_gate("caracterizacion-mutantes", False, f"puntaje de caracterizacion {score} < {minimo} ({muertos}/{len(mutantes)})")
    for t in tests:
        TASK.accept_files[t] = (WT / t).read_text(encoding="utf-8")
    state["accept_files"] = list(TASK.accept_files)
    return rec_gate("caracterizacion-mutantes", True, f"{ids}: verdes en HEAD, puntaje {score} ({muertos}/{len(mutantes)})")


@stage("1-aceptacion")
def aceptacion():
    """Etapa 1 (ticket 12 D5): Claude escribe el test de aceptacion desde la tarea. Gates deterministas: nombra
    test_R<n>_..., compila (collect-only) y FALLA en HEAD (rojo por diseño). Con `approve_accept` (default) la corrida
    se detiene en `awaiting_approval`: un humano (o el agente del ticket 14) aprueba y reanuda desde la etapa 2.
    Con propiedades (ticket 08): cada ID necesita al menos un test rojo; una propiedad puede pasar en HEAD."""
    if TASK.accept_files and not state.get("caracterizacion"):
        return True, "tests de aceptacion provistos por el llamador"
    if state.get("area"):
        return _aceptacion_area()
    ext = _exts()[0]
    rel = str(CFG.get("accept_test", f"{TESTS_PREFIX}test_sdlc_{{slug}}.{ext}")).replace("{slug}", _slug())
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(f"Escribi SOLO el archivo {rel}: el test de aceptacion ({_marco(rel)}, sin red) de esta TAREA. Un test por "
                   f"requisito, nombrados test_R1_..., test_R2_... con el ID en el nombre (funcion o titulo del test); "
                   f"comentario o docstring de modulo con el contrato "
                   f"R<n>. El test DEBE fallar hoy (el codigo aun no existe o no cumple) y pasar cuando la tarea este hecha."
                 f"{_pide_propiedades()} No toques ningun otro archivo.\n\nTAREA:\n{TASK.task}")
    write_supervision(f"aceptacion (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1000]}")
    if not gate_alcance("aceptacion-alcance", before, (rel,))[0] or not (WT / rel).exists():
        return False, f"Claude no dejo {rel} (o toco otros archivos)"
    text = (WT / rel).read_text(encoding="utf-8")
    ids = sorted(set(re.findall(r"(?m)^def test_(R\d+)" if _es_py(rel) else r"\btest_(R\d+)", text)))
    if not ids:
        return rec_gate("aceptacion-ids", False, "sin tests test_R<n>_...")
    perr, state["ids_con_prop"] = propiedades.gate_propiedades(rel, text)
    if perr:
        return rec_gate("aceptacion-propiedades", False, "; ".join(perr))
    if _es_py(rel):
        ok_c, log = sh([PY, "-m", "pytest", rel, "--collect-only", "-q", "-p", "no:cacheprovider"])
        if not ok_c:
            return rec_gate("aceptacion-compila", False, log[-600:])
    # otro lenguaje: el comando del repo compila y corre; debe estar ROJO
    ok_r, log, res = _corre_con_junit([rel] if _es_py(rel) else [])
    if ok_r:
        return rec_gate("aceptacion-roja", False, f"{rel} ya pasa en HEAD: no mide la tarea")
    if state["ids_con_prop"]:
        rojos = _fallan([rel], res, log)
        sin_rojo = [i for i in ids if not any(t.startswith(f"test_{i}_") for t in rojos)]
        if sin_rojo:
            return rec_gate("aceptacion-roja", False, f"{sin_rojo}: ningun test rojo en HEAD (una propiedad sola no mide el cambio)")
    rec_gate("aceptacion-roja", True, f"{rel} roja en HEAD, R: {ids}")
    TASK.accept_files[rel] = text
    ok, note = _parada([rel])
    return ok, note if not ok else f"{rel}: R {ids}, roja en HEAD, sin aprobacion (approve_accept=false)"


def _aceptacion_area() -> tuple[bool, str]:
    """Etapa 1 en modo area (tickets 04 y 05): un archivo por ID en tests/sdlc/<area>/; un ID que ya existe es un
    MODIFIED y uno nuevo toma un numero libre ofrecido. Las bajas (REMOVED) van con motivo en docs/sdlc/bajas.md. Antes
    de la parada, el codigo mueve a removidos/ los tests viejos de cada MODIFIED y de cada baja."""
    area, ext = state["area"], _exts()[0]
    d = f"tests/sdlc/{area}/"
    existentes = [f"R{n}" for n in sorted(_ids_del_area())]
    ofrecidos = [f"R{n}" for n in areas.libres(_ids_del_area(), 5)]
    antes = {r: (WT / r).read_bytes() for rs in areas.archivos_por_id(WT, area).values() for r in rs}
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(f"Escribi los tests de aceptacion ({_marco(f'x.{ext}')}, sin red) de esta TAREA en {d}, UN archivo por "
                 f"requisito: {d}test_R<n>_{_slug()}.{ext}, con tests test_R<n>_... del mismo ID. Un requisito que ya existe "
                 f"en el area ({', '.join(existentes) or 'ninguno'}) y cambia usa su ID; uno nuevo toma el primero libre de "
                 f"{', '.join(ofrecidos)}. Cada ID que escribas necesita al menos un test que FALLE hoy y pase cuando la tarea "
                 f"este hecha.{_pide_propiedades()} Si la tarea elimina requisitos del area, escribi docs/sdlc/bajas.md con "
                 f"una linea por ID: `- R<n>: motivo`. No toques ningun otro archivo.\n\n"
                 f"SPEC DEL AREA ({state['area_spec']}):\n{_spec_area_md()}\n\nTAREA:\n{TASK.task}")
    write_supervision(f"aceptacion (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1000]}")
    nuevos, nota = _nuevos_del_area("aceptacion-alcance", before, ("docs/sdlc/bajas.md",))
    if nuevos is None:
        return False, nota
    bajas = areas.leer_delta("## REMOVED\n" + ((WT / "docs/sdlc/bajas.md").read_text(encoding="utf-8")
                                                if (WT / "docs/sdlc/bajas.md").is_file() else ""))["REMOVED"]
    if not nuevos and not bajas:
        return False, f"Claude no dejo tests en {d} ni bajas"
    errores = _gate_archivos_por_id(nuevos, existentes + ofrecidos)
    errores += [f"baja {i}: no existe en el area" for i in bajas if i not in existentes]
    errores += [f"baja {i} sin motivo" for i, m in bajas.items() if not m]
    if errores:
        return rec_gate("aceptacion-ids", False, "; ".join(errores))
    ids = sorted({re.match(r"test_(R\d+)_", n.rsplit("/", 1)[-1]).group(1) for n in nuevos}, key=areas.numero)  # type: ignore[union-attr]
    if nuevos:
        if all(_es_py(n) for n in nuevos):
            ok_c, log = sh([PY, "-m", "pytest", *nuevos, "--collect-only", "-q", "-p", "no:cacheprovider"])
            if not ok_c:
                return rec_gate("aceptacion-compila", False, log[-600:])
        ok_r, log, res = _corre_con_junit(nuevos if all(_es_py(n) for n in nuevos) else [])
        rojos = _fallan(nuevos, res, log)
        sin_rojo = [i for i in ids if not any(t.startswith(f"test_{i}_") for t in rojos)]
        if sin_rojo or ok_r:
            return rec_gate("aceptacion-roja", False, f"{sin_rojo or ids}: ningun test rojo en HEAD (no mide la tarea)")
    etiqueta, movs = f"{_slug()}-{_sha_corrida()}", []
    for rel, contenido in antes.items():
        rid = re.match(r"test_(R\d+)_", rel.rsplit("/", 1)[-1]).group(1)  # type: ignore[union-attr]
        if rid in ids or rid in bajas:
            tipo, motivo = ("MODIFIED", "reemplazado por un test nuevo") if rid in ids else ("REMOVED", bajas[rid])
            dst = areas.mover(WT, area, rel, rid, tipo, motivo, etiqueta, contenido=contenido if rel in nuevos else None)
            movs.append({"id": rid, "de": rel, "a": dst})
            if rel not in nuevos:
                TASK.accept_files.pop(rel, None)
    state.update(movimientos=movs, bajas=bajas, ids_etapa1=ids)
    rec_gate("aceptacion-roja", True, f"R: {ids}, bajas {sorted(bajas)}, movidos {[m['de'] for m in movs]}")
    for n in nuevos:
        TASK.accept_files[n] = (WT / n).read_text(encoding="utf-8")
    state["ids_con_prop"] = _ids_con_prop([r for rs in areas.archivos_por_id(WT, area).values() for r in rs])
    salen = "".join(f"\n- sale {m['de']} -> {m['a']} ({m['id']})" for m in movs) + \
        "".join(f"\n- baja {i}: {m}" for i, m in bajas.items())
    return _parada(nuevos or ["docs/sdlc/bajas.md"], salen)


def gate_marcas(spec_md: str) -> tuple[bool, str]:
    """Ticket 08: los IDs marcados `[prop]` son exactamente los IDs con test de propiedades (entre los IDs de la spec).
    Sin propiedades y sin marcas, pasa (spec de hoy)."""
    ids = set(areas.requisitos(spec_md)) or set(re.findall(r"\bR\d+\b", spec_md))
    marcados, con_prop = set(propiedades.marcas(spec_md)), set(state.get("ids_con_prop") or []) & ids
    sin_test, sin_marca = sorted(marcados - con_prop), sorted(con_prop - marcados)
    if sin_test or sin_marca:
        return rec_gate("marcas-prop", False, f"marca [prop] sin test de propiedades: {sin_test}; "
                                              f"test de propiedades sin marca [prop]: {sin_marca}")
    return rec_gate("marcas-prop", True, f"marcas {sorted(marcados)}")


def _pide_marcas(ids: list[str]) -> str:
    return f" Marca `[prop]` pegada al ID (`- R<n> [prop]: ...`) exactamente en {ids}, y en ningun otro." if ids else ""


def gate_delta(delta_md: str) -> tuple[bool, str]:
    """Modo area (tickets 04, 05 y 08): el delta contra la spec base del area, sus tests `ruta::test`, los IDs y las
    bajas de la etapa 1 y las marcas. Fusiona por codigo, exige que el diff nombre exactamente los IDs del delta y deja
    la spec fusionada en docs/sdlc/spec.md, que leen el plan y el build."""
    base, area = _spec_area_md(), state["area"]
    errores = areas.gate_delta(base, delta_md, areas.tests_por_id(WT, area))
    d = areas.leer_delta(delta_md)
    cambian, etapa1 = set(d["ADDED"]) | set(d["MODIFIED"]), set(state.get("ids_etapa1") or [])
    if cambian != etapa1:
        errores.append(f"ADDED+MODIFIED {sorted(cambian)} no coincide con los tests de la etapa 1 {sorted(etapa1)}")
    bajas = set(state.get("bajas") or {})
    if set(d["REMOVED"]) != bajas:
        errores.append(f"REMOVED {sorted(d['REMOVED'])} no coincide con las bajas de la etapa 1 {sorted(bajas)}")
    fus = ""
    if not errores:
        fus = areas.regenerar(areas.fusionar(base, delta_md), WT, area)
        errores += areas.chequeo_diff(base, fus, delta_md)
        errores += [f"faltan tokens del contrato {t}" for t in CONTRACT if t not in fus + delta_md]
        ok_m, nota_m = gate_marcas(fus)
        if not ok_m:
            errores.append(nota_m)
    if errores:
        return rec_gate("delta", False, "; ".join(errores))
    _write("docs/sdlc/spec.md", fus)
    return rec_gate("delta", True, f"ADDED {sorted(d['ADDED'])}, MODIFIED {sorted(d['MODIFIED'])}, REMOVED {sorted(d['REMOVED'])}")


def _spec_base() -> tuple[bool, str]:
    """Ticket 03: tras la aprobacion, el writer escribe los requisitos base desde los tests de caracterizacion
    aprobados. El codigo arma la spec del area: Contrato con los archivos caracterizados y Trazabilidad derivada."""
    car, area = state["caracterizacion"], state["area"]
    ids, props = car["ids"], [i for i in state.get("ids_con_prop") or [] if i in car["ids"]]
    tests = "\n\n".join(f"# {t}\n{propiedades.sin_propiedades(t, (WT / t).read_text(encoding='utf-8'))}"
                        for t in car["tests"] if (WT / t).is_file())
    ask = (f"Escribi los requisitos base del area {area}: el comportamiento ACTUAL de {car['archivos']} que fijan estos "
           f"tests de caracterizacion aprobados. Un bloque por ID, exactamente {ids}: `- R<n>: regla`, y debajo, "
           "indentadas, `  Casos borde: ...` y `  Criterio: ...`." + _pide_marcas(props)
           + f" Solo los bloques, sin encabezados.\n\nTESTS:\n{tests}" + _constitucion())
    out, why = "", ""
    for _ in range(2):
        out = llm(WRITER, "Sos un ingeniero de software. Escribis requisitos precisos en markdown, sin relleno.",
                  ask + (f"\n\nTu respuesta anterior fallo el gate: {why}" if why else ""))
        got = areas.requisitos(out)
        why = "; ".join(x for x in (f"IDs {sorted(got)} != {ids}" if sorted(got, key=areas.numero) != ids else "",
                                    f"marcas {propiedades.marcas(out)} != {props}" if propiedades.marcas(out) != props else "") if x)
        if not why:
            break
    if why:
        return rec_gate("spec-base", False, why)
    md = _spec_area_md()
    md = areas.con_requisitos(md, areas.requisitos(md) | areas.requisitos(out))
    _write(state["area_spec"], areas.regenerar(md, WT, area, car["archivos"]))
    return rec_gate("spec-base", True, f"{state['area_spec']}: {ids}")


def _spec_area() -> tuple[bool, str]:
    """Etapa 2 en modo area (ticket 04): el writer escribe SOLO el delta; el codigo lo fusiona y lo chequea."""
    car = state.get("caracterizacion")
    if car and not set(car["ids"]) <= set(areas.requisitos(_spec_area_md())):
        ok, note = _spec_base()
        if not ok:
            return False, note
    base, existentes = _spec_area_md(), set(areas.requisitos(_spec_area_md()))
    etapa1 = state.get("ids_etapa1") or []
    added, modified = [i for i in etapa1 if i not in existentes], [i for i in etapa1 if i in existentes]
    props = [i for i in state.get("ids_con_prop") or [] if i in etapa1]
    ask = (f"Escribi el delta de esta feature sobre la spec del area {state['area']}. Formato OBLIGATORIO, solo estas "
           "secciones (omiti las vacias):\n## ADDED\n- R<n>: <regla>\n  Casos borde: ...\n  Criterio: ...\n## MODIFIED\n"
           "- R<n>: <el requisito ENTERO, con su texto nuevo, casos borde y criterio>\n## REMOVED\n- R<n>: <motivo>\n\n"
           f"ADDED: {added}. MODIFIED: {modified}. REMOVED (con estos motivos): {state.get('bajas') or {}}."
           + _pide_marcas(props) + f"\n\nSPEC DEL AREA:\n{base}\n\nTAREA:\n{TASK.task}\n\n"
           f"TESTS DE ACEPTACION (no se modifican):\n{_tests_text()}\n\nSolo markdown." + _constitucion())
    out = llm(WRITER, "Sos un ingeniero de software. Escribis deltas de specs precisos en markdown, sin relleno.", ask)
    _write("docs/sdlc/delta.md", out)
    ok, note = gate_delta(out)
    if not ok:
        out = llm(WRITER, "Sos un ingeniero. Reescribis el delta completo con el formato obligatorio.",
                  f"Este delta fallo el gate: {note}. Corregilo.\n\n{ask}\n\nDELTA ACTUAL:\n{out}")
        _write("docs/sdlc/delta.md", out)
        ok, note = gate_delta(out)
    return ok, note


@stage("2-spec")
def spec():
    if state.get("area"):
        return _spec_area()
    tpl = (TPL / "spec-template.md").read_text(encoding="utf-8")
    tests = _test_names()
    lang = _lenguaje()   # SSB (TypeScript) recibia "Python 3.12" en la spec
    stack = "Python 3.12, sin dependencias" if lang == "Python" else f"{lang}, sin dependencias que el repo no use ya"
    ask = (f"Escribi spec.md siguiendo EXACTAMENTE esta plantilla (mismas secciones, IDs R<n> unicos, tabla de trazabilidad "
           f"que cita tests por su nombre exacto de entre {sorted(tests)}). {stack}.{_pide_marcas(state.get('ids_con_prop') or [])}\n\n"
           f"PLANTILLA:\n{tpl}\n\nTAREA:\n{TASK.task}\n\nTESTS DE ACEPTACION (no se modifican):\n{_tests_text()}\n\n"
           "Omiti las lineas de instruccion de la plantilla. Sin marcadores pendientes. Solo markdown." + _constitucion())
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.", ask)
    _write("docs/sdlc/spec.md", out)

    def _falla():
        missing = [s for s in CONTRACT if s not in out]
        tok, tnote = gate_traza_spec(out, tests)
        mok, mnote = gate_marcas(out)
        return (f"faltan tokens {missing}; " if missing else "") + ("TBD; " if "TBD" in out else "") + ("" if tok else tnote) \
            + ("" if mok else f"; {mnote}")

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
    guide = (TPL / "spec-review.md").read_text(encoding="utf-8")
    if state.get("area"):
        return _spec_review_area(guide)
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(f"{guide}\n\nTAREA:\n{TASK.task}\n\nEdita SOLO docs/sdlc/spec.md segun estas reglas. NO escribas codigo ni crees "
                 "ni toques ningun otro archivo: la implementacion la hace otra etapa. Responde en una linea cuantas preguntas hiciste.")
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
        _revisor(f"Tu revision de docs/sdlc/spec.md fallo este gate: {note}. Cada R<n> de la spec debe tener un test "
                 f"con su ID en el nombre; NO agregues R nuevos: quitalos o fundilos en un R existente. Edita SOLO docs/sdlc/spec.md.")
        gate_alcance("spec-review-alcance", before, ("docs/sdlc/spec.md",))
        spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
        ok, note = gate_clarificaciones(spec_md)
        if ok:
            ok, note = gate_traza_spec(spec_md, _test_names())
    if ok:
        ok, note = gate_marcas(spec_md)   # ticket 08: la revision no agrega ni quita marcas por su cuenta
    return ok, note


def _spec_review_area(guide: str) -> tuple[bool, str]:
    """Etapa 2b en modo area (ticket 04): Claude lee la spec fusionada como contexto y edita SOLO el delta; las
    clarificaciones quedan en el delta. El gate del delta vuelve a correr despues. Tocar otra cosa rechaza la etapa."""
    note = ""
    for intento in range(2):
        state["claude_calls"] += 1
        before = _tree()
        r = _revisor(f"{guide}\n\nTAREA:\n{TASK.task}\n\nLa spec fusionada del area esta en docs/sdlc/spec.md: leela como "
                     "contexto, NO la edites. Edita SOLO docs/sdlc/delta.md: ahi van `## Clarificaciones` y los cambios a "
                     "los requisitos de ADDED, MODIFIED y REMOVED. NO escribas codigo ni toques ningun otro archivo. "
                     "Responde en una linea cuantas preguntas hiciste."
                     + (f"\n\nTu revision anterior fallo este gate: {note}. Corregi docs/sdlc/delta.md." if intento else ""))
        write_supervision(f"revision del delta (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1500]}")
        ok, note = gate_alcance("spec-review-alcance", before, ("docs/sdlc/delta.md",))
        if not ok:
            return False, note
        delta = (WT / "docs/sdlc/delta.md").read_text(encoding="utf-8")
        ok, note = gate_clarificaciones(delta)
        if ok:
            ok, note = gate_delta(delta)
        if ok:
            return ok, note
    return False, note


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    ext = (CFG.get("ext") or ["py"])[0]   # orchestration-7ys: el ejemplo y la prueba siguen al lenguaje del repo
    prueba = ACCEPT_CMD or CFG.get("accept_cmd") or f"python -m pytest {' '.join(_accept_paths())} -q"
    ask = (f"Escribi plan.md. Formato OBLIGATORIO: seccion '## Archivos' con un item por archivo a escribir, asi:\n"
           f"- `path.{ext}` [R1, R3] [P]\n"
           f"Cada item cita los IDs R<n> de la spec que cubre. Todo R<n> de la spec aparece en algun item. "
           f"[P] marca archivos que NO importan a otros del plan (paralelizables); los demas van en orden de dependencia. "
           f"'## Stack' = UNA linea con el lenguaje, los frameworks y las librerias que usa el cambio (lo que ya usa el "
           f"repo; nada nuevo sin que la spec lo pida). '## Prueba' = `{prueba}`.\n"
           f"Archivos OBLIGATORIOS en '## Archivos': {_need_files()}.\n\nSPEC:\n{spec_md}" + _constitucion())  # D14-diag: el plan omitia test_capas
    # ticket 01: con area, la spec fusionada trae todos los requisitos del area; el plan cubre solo los que cambian
    ids_md = " ".join(state.get("ids_etapa1") or []) if state.get("area") else spec_md
    if state.get("area"):
        ask += (f"\n\nIDs que cambia esta feature (los unicos R<n> que el plan cita; los demas de la spec del area ya "
                f"estan hechos): {state.get('ids_etapa1') or []}. Solo archivos del area {state['area']} o sin area.")
    inf = _informe_spec(spec_md)
    if inf:   # orchestration-e4d: el planner ve los lectores acoplados de lo que la spec nombra
        ask += ("\n\nINFORME DE IMPACTO (otros modulos leen literales de los archivos que nombra la spec; si el cambio "
                f"altera esos literales, sus lectores tambien van en '## Archivos'):\n{inf}")
    out = _planner("Sos un tech lead. Escribis planes ejecutables. NO regeneres los tests.", ask)
    for intento in range(2):  # D2 r1: una reescritura con el motivo del gate, como en spec
        _write("docs/sdlc/plan.md", out)
        m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", out, re.S | re.I)
        block = m.group(1) if m else out
        files = _plan_files(block)
        state["plan_files"] = files
        state["plan_stack"] = _plan_stack(out)   # el rol del coder sale del plan (_rol)
        state["plan_parallel"] = [f for f in files if re.search(rf"{re.escape(f)}`?[^\n]*\[P\]", block)]  # ticket 05 lo ejecuta en paralelo
        ok, note = gate_plan_allowlist(out, files)
        if ok:
            ok, note = gate_traza_plan(ids_md, block, files)
        if ok:
            ok, note = gate_tamano(files)
            if not ok:   # un plan grande no se arregla reescribiendo: hay que dividir la tarea
                _proponer_division(spec_md, out)
                return ok, note + " (propuesta en docs/sdlc/division.md)"
        if ok or intento:
            return ok, note
        out = _planner("Sos un tech lead. Reescribis el plan completo con el formato obligatorio.",
                  f"Este plan fallo el gate: {note}. Corregilo.\n\nPLAN ACTUAL:\n{out}\n\nSPEC:\n{spec_md}")
    return ok, note


def _plan_files(block: str) -> list[str]:
    """Archivos del plan = SOLO los items de la lista (D2 r1: una mencion en prosa 'sin tocar X.py' se colaba) y, en
    una tabla, las rutas entre backticks de la primera celda que nombra alguna (SSB F7a: el plan vino en tabla).
    Un nombre sin carpeta (`resolver.ts`, SSB F4b) pasa a la unica ruta del plan o del repo con ese nombre."""
    items = re.findall(r"(?m)^\s*[-*]\s*`?" + _file_re() + r"`?", _primera_lista(block))
    for fila in _filas_tabla(block):
        celda = next((c for c in fila.split("|") if re.search("`" + _file_re() + "`", c)), "")
        items += re.findall("`" + _file_re() + "`", celda)
    items = [_ruta_unica(f, items) for f in items]
    return [f for f in dict.fromkeys(items) if not f.startswith(TESTS_PREFIX) or f in _need_files()]  # D14-diag: test_capas es obligatorio en cableos


def _primera_lista(block: str) -> str:
    """Solo la PRIMERA lista: una linea en prosa la corta. F2 m1: debajo de "Notas de ejecucion:" venia
    "- `src/tipos.ts` ya existe ... no se modifica" y trazabilidad-plan lo exigia con ID (6 de 12 planes reales)."""
    lineas: list[str] = []
    for linea in block.splitlines():
        if re.match(r"\s*[-*]\s", linea) or (lineas and (not linea.strip() or linea[:1] in " \t")):
            lineas.append(linea)
        elif lineas:
            break
    return "\n".join(lineas)


def _filas_tabla(block: str) -> list[str]:
    return re.findall(r"(?m)^\s*\|(.*)\|\s*$", block)


def _ruta_unica(f: str, listados: list[str]) -> str:
    """`resolver.ts` -> `src/encuadre/resolver.ts` si hay UNA candidata con ese nombre: entre las rutas del plan o, si
    no, entre los archivos del repo dentro del techo. Con cero o varias queda igual y el gate la rechaza por nombre."""
    if "/" in f or (_techo() and _en_techo(f)):
        return f
    cands = {x for x in listados if x.rsplit("/", 1)[-1] == f and "/" in x}
    if not cands and _techo():
        ls = subprocess.run(["git", "ls-files"], cwd=WT, capture_output=True, text=True, encoding="utf-8", errors="replace")
        cands = {x for x in ls.stdout.splitlines() if x.rsplit("/", 1)[-1] == f and "/" in x and _en_techo(x)}
    return cands.pop() if len(cands) == 1 else f


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    state["plan_stack"] = _plan_stack(plan_md)   # tambien al reanudar desde la etapa 4, sin pasar por plan()
    written = {}
    docs_before = _docstrings(state["plan_files"])  # gate docstring-intacto (D4)
    _orig_files.clear()
    _orig_files.update({f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()})
    for f in state["plan_files"]:
        cur = (WT / f).read_text(encoding="utf-8") if (WT / f).exists() else ""
        cur_ctx = f"ARCHIVO ACTUAL {f} (devolvelo COMPLETO con el cambio minimo):\n{cur}\n\n" if cur.strip() else ""
        inf = _informe(f) if cur.strip() else ""   # orchestration-e4d: el coder ve quien lee lo que cambia
        if inf:
            cur_ctx += f"{inf}\n\n"
        out = llm(CODER, _rol(f) + "Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi {f}. Tiene que importar con los archivos ya escritos y pasar los tests de aceptacion.\n\n{cur_ctx}"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nTAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\n"
                  f"ARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = one_file(strip_fence(out), f)
        if code:   # "" = el coder devolvio otro archivo (7b6): queda el actual y el fix loop lo vuelve a pedir
            _write(f, code)
        written[f] = code or cur
        bok, bnote = gate_baseline()
        if not bok:
            return False, bnote
    def _compiles():
        ok, log = gate_compile(state["plan_files"])
        return (ok, log) if not ok else gate_test_compile()

    def _hasta_compilar(ok: bool, log: str) -> tuple[bool, str]:
        """Fix loop del coder x3 y despues la escalera; exito = compila. Corre tras escribir y tras cada re-pedido:
        en SSB F7a2 el re-pedido de cambio-minimo rompio la compilacion y la etapa cortaba sin escalar."""
        for _ in range(3):  # r1 2026-09-11: test-compile tambien entra al fix loop, antes cortaba sin vuelta
            if ok:
                break
            # F2 m3: pedir TODOS reescribia validacion.ts (sano) con el contenido de app.ts, el que tenia el error
            for f in [f for f in state["plan_files"] if f in log] or state["plan_files"]:
                out = llm(CODER, _rol(f) + "Devolves SOLO el codigo corregido del archivo pedido, un solo archivo.",
                          f"py_compile o la importacion de los tests fallo. Corregi {f}.\n\nERROR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
                code = one_file(strip_fence(out), f)
                if code and code != written[f]:
                    _write(f, code); written[f] = code
            ok, log = _compiles()
            if not gate_baseline()[0]:
                return False, "baseline roto en compile-fix"
        if not ok:  # orchestration-7ys: la misma escalera que la etapa 5, con exito = compila
            ok, ladder_note, log = _compile_ladder(log, _compiles)
            if not ok:
                write_supervision(f"ESCALATE_HUMAN: compila: fix 3 + reasoner 2 + Claude agotados.\n{log[-1500:]}")
                return False, f"G3 rechaza tras 3 vueltas y la escalera ({ladder_note}): {log[-300:]}"
        written.update(_all_code())  # la escalera escribe en disco: los gates de abajo miran lo que compila (F7a2)
        return True, "compila"

    ok, log = _compiles()
    if not ok and _toolchain_missing(log):  # orchestration-7ys: sin compilador las vueltas del coder no sirven
        rec_gate("toolchain", False, log[-300:])
        ok, log = _claude_compile(log, _compiles, env=True)
        if not ok and _toolchain_missing(log):
            write_supervision(f"ESCALATE_HUMAN: falta el compilador o una dependencia del entorno y Claude no lo resolvio; "
                              f"su diagnostico queda arriba en esta supervision\n{log[-1500:]}")
            return False, f"toolchain: {log[-300:]}"
    ok, note = _hasta_compilar(ok, log)
    if not ok:
        return False, note
    cok, cnote = gate_clones(written)
    if not cok:
        return False, cnote
    mok, mnote = gate_cambio_minimo(docs_before, written)
    if not mok:  # D11: el coder borro 216 lineas de project_loop.py para agregar 10; una vuelta de re-pedido
        for f in [f for f in written if f in mnote]:
            out = llm(CODER, _rol(f) + "Devolves SOLO el archivo entero.",
                      f"Tu version de {f} borro codigo existente que la tarea NO pide tocar ({mnote}). "
                      f"Parti del ARCHIVO ORIGINAL y aplica SOLO el cambio pedido, conservando todo lo demas.\n\n"
                      f"TAREA:\n{TASK.task}\n\nARCHIVO ORIGINAL:\n{_orig_files.get(f, '')}\n\nTU VERSION:\n{written[f]}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        mok, mnote = gate_cambio_minimo(docs_before, written)
        if not mok:
            return False, f"cambio-minimo: {mnote}"
        ok, note = _hasta_compilar(*_compiles())
        if not ok:
            return False, f"cambio-minimo ok pero no compila: {note}"
        mok, mnote = gate_cambio_minimo(docs_before, written)  # la escalera puede volver a borrar
        if not mok:
            return False, f"cambio-minimo tras la escalera: {mnote}"
    dok, dnote = gate_docstring(docs_before)
    if not dok:  # una vuelta del coder para restaurarlo; si insiste, la etapa falla
        for f in [f for f, d in docs_before.items() if d and not _docstrings([f]).get(f)]:
            out = llm(CODER, _rol(f) + "Devolves SOLO el archivo entero.",
                      f"Restaura el docstring de modulo original de {f} al inicio del archivo, sin cambiar nada mas.\n\n"
                      f"DOCSTRING ORIGINAL:\n\"\"\"{docs_before[f]}\"\"\"\n\nARCHIVO ACTUAL:\n{(WT / f).read_text(encoding='utf-8')}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        dok, dnote = gate_docstring(docs_before)
        if not dok:
            return False, f"docstring: {dnote}"
        ok, note = _hasta_compilar(*_compiles())
        if not ok:
            return False, f"docstring ok pero no compila: {note}"
        dok, dnote = gate_docstring(docs_before)
        if not dok:
            return False, f"docstring tras la escalera: {dnote}"
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
        pick = llm(CODER, _rol() + 'Respondes SOLO un JSON: {"files": [paths], "why": str}.',
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
            out = llm(CODER, _rol(f) + "Devolves SOLO el archivo entero. No toques los tests.",
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
        spok, spnote = gate_sprites()
        if not spok:
            write_supervision(f"ESCALATE_HUMAN: assets invalidos\n{spnote}")
            return False, f"sprites: {spnote[:200]}"
        dok, dnote = gate_codigo_muerto()  # antes del lint: borrar codigo deja imports sin uso y el lint los ve
        if not dok:
            backup = _all_code()
            nuevo = next(f for f in state["plan_files"] if f in dnote)
            out = llm(CODER, _rol(nuevo) + "Devolves SOLO el archivo entero.",
                      f"Ningun test ejecuta estas lineas de {nuevo}. Borra el codigo que la tarea no pide (ramas y formatos "
                      f"que nunca ocurren) sin cambiar el comportamiento que los tests fijan.\n\nMEDIDA:\n{dnote}\n\n"
                      f"TAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(backup)}")
            code = one_file(strip_fence(out), nuevo)
            if code:
                _write(nuevo, code)
            aok, _ = accept()
            dok, dnote = gate_codigo_muerto() if aok else (False, "la aceptacion se rompio al borrar codigo")
            if not dok:
                _revert(backup)
                cok, _ = claude_fix(f"Codigo que ningun test de aceptacion ejecuta; borralo sin romper los tests:\n{dnote}")
                dok, dnote = gate_codigo_muerto() if cok else (False, "Claude no dejo verde la aceptacion")
            if not dok:
                write_supervision(f"ESCALATE_HUMAN: codigo muerto tras coder y Claude\n{dnote}")
                return False, f"codigo muerto: {dnote[:200]}"
        lok, lnote = gate_lint()
        if not lok:  # D2 r1: el pre-commit del repo rechazo 2 errores de mypy; ahora es gate con escalera
            out = llm(CODER, _rol(state["plan_files"][0]) + "Devolves SOLO el archivo entero.",
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
            out = llm(CODER, _rol(state["plan_files"][0]) + "Devolves SOLO el archivo entero.",
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
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    diff = subprocess.run(["git", "diff", "--cached", "--", *state["plan_files"]], cwd=WT,
                          capture_output=True, text=True, encoding="utf-8").stdout
    state["claude_calls"] += 1
    before = _tree()
    r = _revisor(
        "Revisa este diff contra docs/sdlc/spec.md y la TAREA. "
        f"Si encontras un defecto REAL, escribi UN test pytest que falle en {REVIEW_REL} "
        "(una funcion test_*) y responde 'BLOCK: <defecto>'. No toques ningun otro archivo. "
        "Si no hay defecto demostrable con test, no escribas nada y responde 'OK' o 'NOTE: <observacion>'.\n\n"
        f"TAREA:\n{TASK.task}\n\nDIFF:\n{diff[:60000]}")
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
    if state.get("area"):
        ok, note = _archivar_area()
        if not ok:
            return False, note
    else:
        _archivar_spec()
    _commit(f"sdlc: {TASK_NAME} - pipeline 6 etapas, gates + escalera + revision Claude")
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
    # Ticket 1 del mapa portfolio-circuito (2026-09-16): "sin hallazgos NUEVOS por archivo contra la base". Antes exigia
    # cero absoluto en cada archivo tocado, y la deuda vieja de un repo (Portfolio: 730 ruff + 144 mypy) bloqueaba
    # cualquier reparacion. En un repo limpio da lo mismo: la base es cero. Un archivo nuevo parte de cero.
    ahora, log = _hallazgos_por_archivo(files)
    base, _ = _en_base(lambda: _hallazgos_por_archivo([f for f in files if (WT / f).exists()]))
    state["lint_new"] = {f: [base.get(f, 0), ahora.get(f, 0)] for f in files}
    peor = {f: v for f, v in state["lint_new"].items() if v[1] > v[0]}
    if not peor:
        return rec_gate("lint", True, f"sin hallazgos nuevos (base, ahora): {state['lint_new']}")
    return rec_gate("lint", False, f"hallazgos nuevos (base, ahora): {peor}\n{log[-1200:]}")


def _hallazgos_por_archivo(files: list[str]) -> tuple[dict[str, int], str]:
    """ruff + mypy de mmorch sobre `files`; devuelve {archivo: hallazgos} y el log crudo."""
    if not files:
        return {}, ""
    _, rl = sh([sys.executable, "-m", "ruff", "check", "--output-format", "concise", *files], host=True)
    _, ml = sh([sys.executable, "-m", "mypy", "--ignore-missing-imports", *files], host=True)
    cuenta: dict[str, int] = {}
    for m in re.finditer(r"(?m)^(\S+?):\d+:(?:\d+: |\s*error:)", rl + "\n" + ml):
        f = m.group(1).replace("\\", "/")
        cuenta[f] = cuenta.get(f, 0) + 1
    return cuenta, rl + "\n" + ml


def _seed_accept() -> None:
    """Escribe y commitea los tests de aceptacion del llamador (rojos por diseño) si no estan en el arbol."""
    accept = getattr(TASK, "accept_files", {})
    if not accept:
        return
    for rel, content in accept.items():
        f = WT / rel  # tambien refresca uno existente: un accept actualizado (test viejo + feature nueva) no puede quedar stale
        if not f.exists() or f.read_text(encoding="utf-8") != content:
            _write(rel, content)
    subprocess.run(["git", "add", "--", *accept], cwd=WT, check=True)  # solo los tests: el resto del arbol no es nuestro
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=WT).returncode != 0:
        subprocess.run(["git", "-c", "user.name=mmorch", "-c", "user.email=mmorch@local", "commit", "-q", "-m",
                        f"sdlc: test de aceptacion {TASK_NAME} (rojo por diseño)"], cwd=WT, check=True)


# Claves del run-log que una corrida reanudada recupera (SDD fase 2): area, movimientos y bajas de la etapa 1.
_PERSISTE = ("area", "area_spec", "area_archivos", "accept_files", "movimientos", "bajas", "ids_etapa1", "ids_con_prop",
             "caracterizacion", "sha_corrida", "fuera_de_area")


def run(from_stage: float = 2, to_stage: float = 6) -> dict:
    """Corre las etapas desde `from_stage` hasta `to_stage` (2 spec, 2.5 spec-review, 3 plan, 4 build, 5 test, 5.5 review,
    6 pr). La etapa es el checkpoint (ticket 05 D4): reanudar = mismo worktree + from_stage. `to_stage` < 6 corta antes
    del PR (orchestration-34k: repetir una corrida real sin revision ni PR)."""
    _RUN_USD["usd"] = 0.0
    register_run_tracker(_RUN_USD)  # W3.4: tope USD por-run medido en proceso, no solo por el ledger
    try:
        return _run_stages(from_stage, to_stage)
    finally:
        unregister_run_tracker(_RUN_USD)


def _run_stages(from_stage: float, to_stage: float = 6) -> dict:
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
        else:  # directorio plano ya materializado por el llamador (bench de mmorch, workflow_race): el pipeline necesita git
            subprocess.run(["git", "init", "-q"], cwd=WT, check=True)
            subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
            subprocess.run(["git", "-c", "user.name=mmorch", "-c", "user.email=mmorch@local", "commit", "-q", "-m", "base"], cwd=WT)
        print("materializado", WT, flush=True)
    previo: dict = {}
    if LOG.exists():
        try:
            previo = json.loads(LOG.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            previo = {}
    if not TASK.accept_files:  # resume tras awaiting_approval: los tests ya viven en la branch
        for rel in [previo.get("awaiting_approval"), *previo.get("accept_files", [])]:
            if rel and (WT / rel).is_file():
                TASK.accept_files[rel] = (WT / rel).read_text(encoding="utf-8")
    if from_stage > 1:   # area, movimientos y bajas de la etapa 1 siguen valiendo: una reanudacion no los repite
        state.update({k: previo[k] for k in _PERSISTE if k in previo})
    _seed_accept()  # tambien sobre un worktree abierto por el server: el test de aceptacion entra commiteado
    state["base_sha"] = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip() \
        if from_stage <= 3 else "HEAD~1"  # en resume no se conoce: HEAD~1 como antes
    state.setdefault("sha_corrida", state["base_sha"] if from_stage <= 1 else _sha_corrida())
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
    stages = {0: caracterizar, 1: aceptacion, 2: spec, 2.5: spec_review, 3: plan, 4: build, 5: test, 5.5: review, 6: pr}
    inicio = 0 if from_stage == 1 else from_stage   # la etapa 0 corre siempre antes de la 1 (sin area no hace nada)
    try:
        if from_stage <= 1:
            _resolver_area()
        _preparar_propiedades()
        for k in sorted(stages):
            if inicio <= k <= to_stage:
                stages[k]()
                if k == 5.5 and state.get("review_block"):
                    test()  # una vuelta mas con el test de Claude; si sigue rojo, escala
    except StageFailed as e:  # la etapa es el checkpoint: el run-log dice donde reanudar
        state["failed_stage"], state["failed_note"] = e.stage, e.note
        print(str(e), flush=True)
    state["minutes_total"] = round((time.time() - state["t0_epoch"]) / 60, 2)
    state["usd"] = ledger_usd()
    state["usd_run"] = round(_RUN_USD["usd"], 4)  # antes del flush y del return: en el finally de run() llegaba tarde
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
    # ponytail: la corrida vive en globals del modulo; dos jobs a la vez se pisaban WT (Estudio leyo el worktree de
    # Portfolio). Lock global = una feature por proceso; estado por instancia si hace falta paralelismo.
    with _RUN_LOCK:
        configure(t, contract=feat["contract"], feat=feat, wt=wt, phase=phase, max_fix=max_fix, writer=writer, coder=coder,
                  accept_cmd=accept_cmd)
        try:  # repos-convenciones 03: el estado del repo queda en el run-log y ningun desvio frena la corrida
            from .convenciones import doctor
            state["doctor"] = doctor(repo)
        except Exception as e:  # noqa: BLE001 - un doctor roto no puede frenar un build
            state["doctor"] = {"error": f"{type(e).__name__}: {e}"}
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
    log = RUNS / "veredictos.jsonl"
    # 2026-09-17: cada relanzamiento con `verdict` re-registraba la misma decision (17 filas para 7): un veredicto es
    # (repo, branch, test, etiqueta); repetido, no suma un ejemplo nuevo al umbral del checker en sombra.
    clave = _clave_veredicto(rec)
    for linea in (log.read_text(encoding="utf-8").splitlines() if log.exists() else []):
        if linea.strip() and _clave_veredicto(json.loads(linea)) == clave:
            return json.loads(linea)
    with log.open("a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    return rec


def _clave_veredicto(rec: dict) -> tuple:
    return rec.get("repo"), rec.get("branch"), rec.get("test_rel"), rec.get("test"), rec.get("label")


def init(repo: str) -> dict:
    """Ticket 12: un repo entra al pipeline. Escribe `sdlc.toml` (techo = fuentes .py fuera de tests; accept_cmd; suite;
    approve_accept), `docs/sdlc/` con las plantillas y un puntero en AGENTS.md. Nunca pisa lo que ya existe."""
    root = pathlib.Path(repo).resolve()
    hecho: list[str] = []
    toml = root / "sdlc.toml"
    if not toml.exists():
        exts, globs = _globs_iniciales(root)
        hints = "\n".join(f"# compile_cmd = \"{_LANG_HINT[e]}\"   # {e}" for e in exts if e != "py")
        toml.write_text(
            "# sdlc.toml — convencion por repo (ticket 11). `files` es el TECHO (patrones) de lo que el pipeline puede\n"
            "# escribir; un payload solo lo acota. Sin accept_cmd el repo no entra. approve_accept: un humano aprueba\n"
            "# el test de aceptacion que escribe la etapa 1 antes de gastar (ticket 12 D5).\n"
            "# Otros lenguajes: `ext`, `compile_cmd` (G3 y test-compile), `lint_cmd` ({files} = archivos del plan),\n"
            "# `suite_cmd` (suite total si no es pytest), `seed_globs` (ignorados que el worktree necesita), `accept_test` ({slug}).\n"
            "# `constitution`: reglas fijas que leen spec y plan (default GOAL.md; sin el archivo, nada).\n"
            "# Umbrales: `cobertura_min` (0.8; fraccion de un .py NUEVO que corre la aceptacion), `mutation_min` (sin: observa).\n"
            "# [areas] nombre = [globs]: spec viva por area en docs/specs/<area>/spec.md (o [spec_de_area] nombre = \"ruta\"),\n"
            "# deltas en docs/specs/<area>/changes/; `caracterizacion_min` (sin: basta un mutante muerto).\n"
            "# `reviewer_cmd`: agente que revisa y corrige (prompt por stdin, `{modo}` = plan|edit); sin esta clave, `claude -p`.\n"
            "# [sprites]: dir, lado, paleta, tope_colores, hitboxes, animaciones, referencias. Lo determinista bloquea;\n"
            "# el juez visual de `referencias` solo observa hasta kappa 0.6 sobre 50 sprites etiquetados (ticket 15).\n"
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
    if ag.exists() and "## SDLC (pipeline de 6 etapas)" not in ag.read_text(encoding="utf-8"):
        from .convenciones import puntero_marcado  # el mismo bloque con marcas que mantiene `init --upgrade`
        ag.write_text(ag.read_text(encoding="utf-8").rstrip("\n") + "\n\n" + puntero_marcado(), encoding="utf-8")
        hecho.append("AGENTS.md")
    return {"repo": str(root), "hecho": hecho}


SKIP_INIT = {"tests", "tests_accept", ".venv", "venv", "node_modules", "build", "dist", "docs", "target", "__pycache__"}


def _globs_iniciales(root: pathlib.Path) -> tuple[list[str], list[str]]:
    """Lenguajes (hasta 3, por cantidad de archivos) y techo `files` por directorio, como los escribe `init`."""
    conteo: dict[str, int] = {}
    dirs: dict[str, set[str]] = {}
    sueltos: dict[str, set[str]] = {}  # en `_calza()` el `*` cruza carpetas: `*.py` abriria el techo a todo el repo
    ls = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, encoding="utf-8")
    # repo git: solo lo trackeado (Portfolio tenia _kimi_exp/ y _tmp_*.py sin trackear); si no, el arbol entero
    for q in ([root / x for x in ls.stdout.splitlines()] if ls.returncode == 0 and ls.stdout else root.rglob("*")):
        if not q.is_file():
            continue
        rel = q.relative_to(root)
        if any(part in SKIP_INIT or part.startswith(".") for part in rel.parts):
            continue
        ext = q.suffix.lstrip(".").lower()
        if ext in _LANG_HINT:
            conteo[ext] = conteo.get(ext, 0) + 1
            if len(rel.parts) > 1:
                dirs.setdefault(ext, set()).add(rel.parts[0])
            else:
                sueltos.setdefault(ext, set()).add(rel.name)
    exts = [e for e, _ in sorted(conteo.items(), key=lambda kv: -kv[1])][:3] or ["py"]
    globs = {f"{d}/**/*.{e}" for e in exts for d in dirs.get(e, set())} | {f for e in exts for f in sueltos.get(e, set())}
    return exts, sorted(globs) or [f"*.{exts[0]}"]  # repo sin codigo: el techo nace abierto hasta el primer archivo


def main(argv: list[str] | None = None) -> int:
    """CLI: `python -m sdlc init [--upgrade]|doctor|areas|propiedades|bd|lint-md <repo>` y
    `python -m sdlc specs --chequear|--regenerar [repo]` (pre-commit de sdd-fase-2 ticket 06: chequear nunca escribe).
    El modo bench vive en mmorch (`python -m mmorch.sdlc_bench`)."""
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["specs"] and argv[1:2] in (["--chequear"], ["--regenerar"]):
        repo = argv[2] if len(argv) > 2 else "."
        if argv[1] == "--regenerar":
            print(json.dumps({"regeneradas": areas.regenerar_repo(repo)}, ensure_ascii=False))
            return 0
        errores = areas.chequear(repo)
        print("\n".join(errores) if errores else "specs al dia")
        return 1 if errores else 0
    if argv[:1] in (["bd"], ["lint-md"]):
        from . import agentes
        return agentes.main(argv)
    if argv[:1] in (["doctor"], ["areas"], ["propiedades"]) or argv[:2] == ["init", "--upgrade"]:
        from . import convenciones
        return convenciones.main(argv)
    if argv[:1] != ["init"]:
        print("uso: python -m sdlc init [--upgrade]|doctor|areas|propiedades|bd|lint-md <repo> | "
              "python -m sdlc specs --chequear|--regenerar [repo]", file=sys.stderr)
        return 2
    print(json.dumps(init(argv[1] if len(argv) > 1 else "."), ensure_ascii=False))
    return 0
