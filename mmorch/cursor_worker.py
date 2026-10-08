"""cursor_worker — despachador de Cursor como trabajador de Claude (orchestration-ayz).

Protocolo: mapa `cursor-trabajador`, tickets 03, 05 y 06 (`.scratch/cursor-trabajador/`). Claude escribe el pedido y
corre este comando en segundo plano: Claude Code avisa al terminar. Claude lee el diff, corre los tests en el
worktree y decide: aplicar, corregir una sola vez o descartar. Cursor trabaja en un worktree descartable; al repo
solo vuelve su diff, y solo con `aplicar`.

Medido en Windows (cursor-agent 2026.10.01, 2026-10-04): los hooks del workspace no corren sin interfaz, `--sandbox`
no existe, y la herramienta de edicion escribe fuera del worktree sin que ninguna regla `Write(ruta)` la frene. Por
eso: sin `--force` (la terminal solo corre la lista permitida), `deny Mcp(*:*)`, y una huella de lo sensible antes y
despues de cada vuelta. Si algo cambia, la corrida queda en ALERTA para siempre y `aplicar` se niega.

Uso:
  python -m mmorch.cursor_worker correr <repo> <pedido.md> [--modelo M]
  python -m mmorch.cursor_worker corregir <run_id> <pedido.md>
  python -m mmorch.cursor_worker aplicar <run_id> [--permitir-borrados] [--permitir-protegidos] [motivo]
  python -m mmorch.cursor_worker descartar <run_id> [motivo]
"""
from __future__ import annotations

import fnmatch
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid
from pathlib import Path
from typing import Any

from . import canal
from .paths import logs_dir

MODELO = "grok-4.7-medium"          # ticket 04: 24/24 en tareas acopladas
RESPALDO = "gpt-5.4-mini-medium"    # ticket 05: despues de un reintento con el mismo modelo
TIMEOUT_S = 20 * 60                 # ticket 05: tope de 20 minutos por vuelta, reintentos incluidos
DENY_REPOS = ("Portfolio financiero", "Portfolio-financiero")   # el codigo de Portfolio nunca va a Cursor
SIN_SHELL = ("SHELL", "BASH", "BASH_ENV", "MSYSTEM", "CLAUDECODE")   # con el bash de Git, Cursor rompe los hooks
SECRETOS = (".env", ".env.*", "*.pem", "*.key", "*.p12", "*.pfx", "*.jks", "*.kdbx", "*credential*", "*secret*",
            "id_rsa*", "id_ed25519*", "token.json", ".npmrc", ".pypirc", ".netrc")
# Config que da ejecucion o escala permisos: en cualquier nivel del repo (Claude Code carga .claude/ por directorio).
DIRS_PROTEGIDOS = {".claude", ".cursor", ".git", ".husky", ".vscode", ".idea"}
NOMBRES_PROTEGIDOS = {"agents.md", "claude.md", "gemini.md", ".mcp.json", "sdlc.toml", ".cursorrules",
                      ".pre-commit-config.yaml"}
TERMINAL = ("python -m pytest", "pytest", "npm test", "npm run test", "npx tsc", "node --test", "mvn test", "mvn -q test",
            "gradle test", "go test", "cargo test", "git status", "git diff", "git log", "git show")
CLI_JSON = ".cursor/cli.json"
DIFF = ("--no-ext-diff", "--no-color", "--src-prefix=a/", "--dst-prefix=b/")   # sin la config de diff del usuario
REGLAS = """REGLAS (trabajas para Claude, que revisa tu diff antes de integrarlo):
- Trabaja SOLO dentro de esta carpeta. Nunca escribas fuera de ella.
- No hagas commit, push ni merge. No instales ni descargues nada. No borres archivos que el pedido no nombre.
- No toques credenciales (.env, claves) ni configuracion de agentes (.claude/, .cursor/, AGENTS.md, CLAUDE.md).
- La terminal solo corre tests y git de lectura. Si necesitas algo prohibido, no lo intentes por otra via: anotalo.
- Al terminar responde con este formato:
RESUMEN: 2 a 4 oraciones.
ARCHIVOS: a.py, b.py
ACCIONES BLOQUEADAS: lo que necesitabas y no pudiste hacer, o "ninguna"."""


class Rechazo(Exception):
    """El despachador se niega (repo prohibido, ALERTA, borrados o protegidos sin permiso, segunda correccion)."""


# ---------------------------------------------------------------- rutas y blancos (los tests los redirigen)
def runs_dir() -> Path:
    return logs_dir() / "cursor_runs"


def log_path() -> Path:
    return logs_dir() / "cursor_runs.jsonl"


def wt_root() -> Path:
    return Path(tempfile.gettempdir()) / "cursor-worker"


def sensibles(repo: Path) -> list[Path]:
    """Archivos sueltos que Cursor nunca debe tocar fuera del worktree (sha256 del contenido)."""
    h = Path.home()
    docs = h / "Documents"
    fijos = [h / ".claude" / "settings.json", h / ".claude" / "settings.local.json", h / ".claude" / "never-edit.txt",
             h / ".claude" / "CLAUDE.md", h / ".cursor" / "hooks.json", h / ".cursor" / "mcp.json", h / ".gitconfig",
             h / ".bashrc", h / ".bash_profile", h / ".profile",
             docs / "WindowsPowerShell" / "Microsoft.PowerShell_profile.ps1",
             docs / "PowerShell" / "Microsoft.PowerShell_profile.ps1",
             repo / ".git" / "config", repo / "AGENTS.md", repo / "CLAUDE.md", repo / "sdlc.toml"]
    globs = [(h / ".claude" / "hooks", "**/*"), (h / ".claude", "*.ps1"), (h / ".ssh", "*"),
             (repo / ".git" / "hooks", "*"), (repo, ".env*")]
    out = [p for p in fijos if p.is_file()]
    for base, pat in globs:
        if base.is_dir():
            out += [p for p in base.glob(pat) if p.is_file()]
    return sorted(set(out))


def claves_json() -> list[tuple[Path, str]]:
    """Archivos que su dueno reescribe seguido: solo cuenta la clave que da permisos o servidores MCP."""
    h = Path.home()
    return [(h / ".cursor" / "cli-config.json", "permissions"), (h / ".claude.json", "mcpServers")]


def arboles(repo: Path) -> list[Path]:
    """Carpetas con codigo que Claude o el repo ejecutan: huella por (ruta, tamano, mtime), sin leer contenido."""
    h = Path.home()
    out = [h / ".claude" / n for n in ("skills", "agents", "commands", "scripts", "plugins")]
    out += [h / ".mmorch", repo / ".claude", repo / ".cursor"]
    return [p for p in out if p.is_dir()]


# ---------------------------------------------------------------- utilidades
def _git(repo: Path, *args: str, check: bool = True) -> str:
    r = subprocess.run(["git", "-c", "core.quotepath=off", "-C", str(repo), *args], capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if check and r.returncode:
        raise RuntimeError(f"git {' '.join(args[:3])}: {r.stderr.strip()[-300:]}")
    return r.stdout


def _git_bytes(repo: Path, *args: str, stdin: bytes | None = None) -> bytes:
    r = subprocess.run(["git", "-c", "core.quotepath=off", "-C", str(repo), *args], input=stdin, capture_output=True)
    if r.returncode:
        raise RuntimeError(f"git {' '.join(args[:3])}: {r.stderr.decode('utf-8', 'replace').strip()[-300:]}")
    return r.stdout


def _huella(d: Path, excluir: str = "") -> str:
    h = hashlib.sha256()
    for raiz, dirs, files in os.walk(d):   # sin seguir junctions ni symlinks
        dirs[:] = sorted(x for x in dirs if x != excluir and not os.path.islink(os.path.join(raiz, x))
                         and not os.path.isjunction(os.path.join(raiz, x)))
        for f in sorted(files):
            p = os.path.join(raiz, f)
            try:
                s = os.stat(p)
            except OSError:
                continue
            h.update(f"{os.path.relpath(p, d)}|{s.st_size}|{s.st_mtime_ns}\n".encode())
    return h.hexdigest()


def _foto(r: dict) -> dict[str, str]:
    """Huella de todo lo que Cursor nunca debe tocar fuera del worktree (incluye otras corridas y los sembrados)."""
    repo = Path(r["repo"])
    out = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sensibles(repo) if p.is_file()}
    for p, clave in claves_json():
        try:
            out[f"{p}#{clave}"] = hashlib.sha256(json.dumps(json.loads(p.read_text(encoding="utf-8")).get(clave),
                                                            sort_keys=True).encode()).hexdigest()
        except (OSError, ValueError, AttributeError):
            continue
    for d in arboles(repo):
        out[str(d)] = _huella(d)
    if runs_dir().is_dir():   # otra corrida pendiente: Cursor podria editar su diff o su resumen
        out[str(runs_dir())] = _huella(runs_dir(), excluir=r["run_id"])
    for real in r.get("sembrados_reales") or []:   # node_modules del repo real, enlazado en el worktree
        out[real] = _huella(Path(real))
    return out


def _cambiados(antes: dict[str, str], despues: dict[str, str]) -> list[str]:
    return sorted(k for k in set(antes) | set(despues) if antes.get(k) != despues.get(k))


def _repo_estado(repo: Path) -> dict[str, str]:
    """Hash de cada archivo con cambios en el repo real: avisa si algo lo toco durante la corrida."""
    tok = _git(repo, "status", "--porcelain=v1", "-z", "--untracked-files=all").split("\0")
    rels, i = [], 0
    while i < len(tok):
        e = tok[i]
        if len(e) > 3:
            rels.append(e[3:])
            i += 1 if e[0] not in "RC" else 2   # rename/copia: el siguiente token es el origen
        else:
            i += 1
    return {r: (hashlib.sha256((repo / r).read_bytes()).hexdigest() if (repo / r).is_file() else "-") for r in rels}


def _secreto(rel: str) -> bool:
    return any(fnmatch.fnmatch(Path(rel).name.lower(), p) for p in SECRETOS)


def _protegido(rel: str) -> bool:
    partes = [p.lower() for p in Path(rel).parts]
    return (any(p in DIRS_PROTEGIDOS for p in partes[:-1]) or partes[-1] in NOMBRES_PROTEGIDOS or _secreto(rel)
            or "/".join(partes).startswith(".github/workflows/"))


def _rmtree(d: Path) -> None:
    def escribible(f, x, e):   # git deja .git/objects de solo lectura en Windows
        os.chmod(x, stat.S_IWRITE)
        f(x)
    shutil.rmtree(d, onexc=escribible)


def cursor_argv() -> list[str]:
    """node + index.js de la version mas nueva de cursor-agent (el .cmd pasa por PowerShell y corta un prompt multilinea).
    ponytail: en Linux se asume la carpeta del instalador oficial (~/.local/share/cursor-agent); verificar si cambia."""
    base = Path(os.environ.get("LOCALAPPDATA", "")) if os.name == "nt" else Path.home() / ".local" / "share"
    vs = base / "cursor-agent" / "versions"
    v = max((p for p in vs.iterdir() if re.match(r"\d{4}\.\d+\.\d+-", p.name)),
            key=lambda p: tuple(int(x) for x in p.name.split("-")[0].split(".")))
    return [str(v / ("node.exe" if os.name == "nt" else "node")), str(v / "index.js")]


def run_agent(argv: list[str], cwd: Path, prompt: str | None, env: dict, timeout: float) -> str:
    """Corre un agente. Timeout o rc != 0 = falla tecnica (RuntimeError), con el arbol entero muerto: cursor-agent
    lanza hijos (node, rg) que un kill simple deja vivos en Windows."""
    p = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.PIPE if prompt is not None else subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace",
                         env=env)
    try:
        out, err = p.communicate(prompt, timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(p.pid)], capture_output=True)
        else:
            p.kill()
        p.communicate()
        raise RuntimeError(f"agente: timeout de {timeout:.0f} s") from None
    if p.returncode:
        raise RuntimeError(f"agente: rc={p.returncode} {(err or out)[-300:]}")
    return out


def terminal_permitida(repo: Path) -> list[str]:
    """Shell permitido sin --force: runners de tests, git de lectura y los comandos del sdlc.toml del repo."""
    cmds = list(TERMINAL)
    toml = repo / "sdlc.toml"
    if toml.is_file():
        cfg = tomllib.loads(toml.read_text(encoding="utf-8"))
        cmds += [str(cfg.get(k) or "").split("{files}")[0].strip() for k in ("accept_cmd", "compile_cmd", "suite_cmd")]
    return [f"Shell({c}*)" for c in dict.fromkeys(c for c in cmds if c)]


def _prohibido(repo: Path) -> bool:
    """Por la carpeta, el repo principal de un worktree y la URL del remoto: un worktree de Portfolio en %TEMP% o un
    clon con otro nombre tambien quedan afuera."""
    deny = DENY_REPOS + tuple(x for x in os.environ.get("MMORCH_CURSOR_DENY", "").split(";") if x)
    ids = [str(repo), _git(repo, "remote", "get-url", "origin", check=False).strip()]
    common = _git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir", check=False).strip()
    if common:
        ids.append(str(Path(common).parent))
    return any(d.lower() in i.lower() for d in deny for i in ids if i)


# ---------------------------------------------------------------- worktree
def _sembrar(repo: Path, wt: Path) -> list[str]:
    """seed_globs del sdlc.toml (node_modules) como junction: devuelve las rutas reales enlazadas."""
    toml = repo / "sdlc.toml"
    reales = []
    for pat in (tomllib.loads(toml.read_text(encoding="utf-8")).get("seed_globs", []) if toml.is_file() else []):
        src, dst = repo / pat, wt / pat
        if src.is_dir() and not dst.exists():
            subprocess.run(["cmd", "/c", "mklink", "/J", str(dst), str(src)] if os.name == "nt"
                           else ["ln", "-s", str(src), str(dst)], capture_output=True)
            if dst.exists():
                reales.append(str(src))
    return reales


def _desenlazar(wt: Path) -> None:
    """Quita junctions y symlinks del worktree SIN recorrerlos: `git worktree remove` los sigue y vacia el destino
    (medido con git 2.53 en Windows: borraba el node_modules real)."""
    if not wt.is_dir():
        return
    for raiz, dirs, _ in os.walk(wt):
        for x in list(dirs):
            p = os.path.join(raiz, x)
            if os.path.isjunction(p) or os.path.islink(p):
                os.rmdir(p) if os.path.isjunction(p) else os.unlink(p)
                dirs.remove(x)
            elif x == ".git":
                dirs.remove(x)


def _abrir_worktree(repo: Path, run_id: str) -> tuple[Path, str, list[str]]:
    """Worktree en HEAD + los cambios sin commitear del repo (sin secretos). Devuelve (wt, arbol base, sembrados)."""
    wt = wt_root() / run_id
    wt.parent.mkdir(parents=True, exist_ok=True)
    _git(repo, "worktree", "add", "-q", "--detach", str(wt), "HEAD")
    try:
        patch = _git_bytes(repo, "diff", "HEAD", "--binary", *DIFF)
        if patch.strip():
            _git_bytes(wt, "apply", "--whitespace=nowarn", "-", stdin=patch)
        for rel in filter(None, _git(repo, "ls-files", "--others", "--exclude-standard", "-z").split("\0")):
            src = repo / rel
            if src.is_file() and not _secreto(rel):   # un repo anidado sale como 'dir/': no se copia
                (wt / rel).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src, wt / rel)
        sembrados = _sembrar(repo, wt)
        _git(wt, "add", "-A")
        base = _git(wt, "write-tree").strip()
        cli = wt / CLI_JSON
        cli.parent.mkdir(exist_ok=True)
        cli.write_text(json.dumps({"permissions": {"allow": terminal_permitida(repo), "deny": ["Mcp(*:*)"]}}),
                       encoding="utf-8")
        return wt, base, sembrados
    except BaseException:
        _cerrar_worktree(repo, wt)
        raise


def _cerrar_worktree(repo: Path, wt: Path) -> None:
    _desenlazar(wt)
    _git(repo, "worktree", "remove", "--force", str(wt), check=False)
    if wt.exists():
        _rmtree(wt)
    _git(repo, "worktree", "prune", check=False)


# ---------------------------------------------------------------- cursor
def _cursor(wt: Path, prompt: str, modelo: str, resume: str | None, timeout: float) -> dict:
    env = {k: v for k, v in os.environ.items() if k not in SIN_SHELL}
    argv = cursor_argv() + ["-p", "--trust", "--workspace", str(wt), "--model", modelo, "--output-format", "stream-json"]
    if resume:
        argv += ["--resume", resume]
    out = run_agent(argv + [prompt], wt, None, env, timeout)
    res: dict = {"result": "", "session_id": None, "tokens": 0, "bloqueadas": [], "herramientas": {}}
    for line in out.splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        res["session_id"] = res["session_id"] or ev.get("session_id")
        if ev.get("type") == "result":
            res["result"] = str(ev.get("result") or "")
            u = ev.get("usage") or {}
            res["tokens"] = (u.get("inputTokens") or 0) + (u.get("outputTokens") or 0)
        elif ev.get("type") == "tool_call" and ev.get("subtype") == "completed":
            for k, v in (ev.get("tool_call") or {}).items():
                if not k.endswith("ToolCall"):
                    continue
                res["herramientas"][k] = res["herramientas"].get(k, 0) + 1
                r = (v or {}).get("result") or {}
                for motivo in ("rejected", "permissionDenied"):
                    if motivo in r:
                        d = r[motivo] or {}
                        res["bloqueadas"].append(str(d.get("command") or d.get("path") or d.get("error") or k)[:200])
    if not res["result"]:
        raise RuntimeError(f"cursor: sin result: {out[-300:]}")
    return res


def _cursor_con_respaldo(wt: Path, prompt: str, modelo: str, resume: str | None) -> tuple[dict, str, int]:
    """Ticket 05: un reintento con el mismo modelo y despues el de respaldo, todo dentro de TIMEOUT_S por vuelta.
    Una falla tecnica no consume la vuelta de correccion."""
    fin, err = time.time() + TIMEOUT_S, ""
    for i, m in enumerate((modelo, modelo, RESPALDO)):
        resta = fin - time.time()
        if resta < 60:
            break
        try:
            return _cursor(wt, prompt, m, resume, resta), m, i + 1
        except RuntimeError as e:
            err = str(e)
    raise RuntimeError(f"cursor fallo dentro del tope de {TIMEOUT_S // 60} min: {err}")


# ---------------------------------------------------------------- corridas
def _resumen_path(run_id: str) -> Path:
    return runs_dir() / run_id / "resumen.json"


def _cargar(run_id: str) -> dict:
    p = _resumen_path(run_id)
    if not p.is_file():
        raise Rechazo(f"no existe la corrida {run_id}")
    return json.loads(p.read_text(encoding="utf-8"))


def _guardar(r: dict, evento: str, **extra) -> None:
    p = _resumen_path(r["run_id"])
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(r, ensure_ascii=False, indent=1), encoding="utf-8")
    fila = {"ts": time.strftime("%Y-%m-%dT%H:%M:%S"), "evento": evento,
            **{k: r.get(k) for k in ("run_id", "repo", "modelo", "modelo_usado", "cli_version", "estado", "secs",
                                     "tokens", "intentos", "archivos", "bloqueadas", "alerta", "correcciones", "error")},
            **extra}
    log_path().parent.mkdir(parents=True, exist_ok=True)
    with log_path().open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(fila, ensure_ascii=False) + "\n")


def _trabajar(r: dict, prompt: str, resume: str | None = None) -> dict:
    """Una vuelta de Cursor: huella, Cursor, huella, diff. La huella y el diff se calculan SIEMPRE, tambien si la vuelta
    falla, y la ALERTA es acumulativa: una vuelta limpia nunca borra la de una vuelta anterior."""
    repo, wt = Path(r["repo"]), Path(r["wt"])
    antes, repo_antes = _foto(r), _repo_estado(repo)
    t0, err = time.time(), None
    res: dict = {"result": "", "session_id": None, "tokens": 0, "bloqueadas": [], "herramientas": {}}
    modelo, intentos = r["modelo"], 0
    try:
        res, modelo, intentos = _cursor_con_respaldo(wt, prompt, r["modelo"], resume)
    except RuntimeError as e:
        err = e
    secs = round(time.time() - t0, 1)
    r["alerta"] = sorted(set(r.get("alerta") or []) | set(_cambiados(antes, _foto(r))))
    r["cambios_en_repo"] = sorted(set(r.get("cambios_en_repo") or []) | set(_cambiados(repo_antes, _repo_estado(repo))))
    _git(wt, "add", "-A")
    spec = ["--", ".", f":(exclude){CLI_JSON}"]
    patch = _git_bytes(wt, "diff", "--cached", "--binary", "--no-renames", *DIFF, r["base"], *spec)
    tok = _git(wt, "diff", "--cached", "--no-renames", "--name-status", "-z", r["base"], *spec).split("\0")
    estado = [(tok[i], tok[i + 1]) for i in range(0, len(tok) - 1, 2)]
    (runs_dir() / r["run_id"] / "diff.patch").write_bytes(patch)
    archivos = [p for _, p in estado]
    r.update({"modelo_usado": modelo, "intentos": intentos, "secs": round(r.get("secs", 0) + secs, 1),
              "tokens": r.get("tokens", 0) + res["tokens"], "session_id": res["session_id"] or r.get("session_id"),
              "resultado": res["result"][-3000:], "bloqueadas": res["bloqueadas"], "herramientas": res["herramientas"],
              "archivos": archivos, "borrados": [p for s, p in estado if s.startswith("D")],
              "protegidos": [p for p in archivos if _protegido(p)], "diff": str(runs_dir() / r["run_id"] / "diff.patch"),
              "error": str(err)[-600:] if err else None,
              "estado": "alerta" if r["alerta"] else "error_tecnico" if err else "listo"})
    try:   # contabilidad del plan de Cursor junto al resto (mapa cursor-trabajador: zona sin especificar)
        from .metrics import log_event
        log_event(pattern="cursor_worker", node=r["run_id"], model=modelo, family="cursor", in_tokens=res["tokens"],
                  out_tokens=0, cost_usd=0.0, latency_s=secs, phase="cursor_worker")
    except Exception:  # ponytail: la metrica nunca rompe la corrida
        pass
    if err:
        raise err
    return r


def correr(repo: str | Path, pedido: str, modelo: str = MODELO) -> dict:
    repo = Path(repo).resolve()
    if not (repo / ".git").exists():
        raise Rechazo(f"no es un repo git: {repo}")
    if _prohibido(repo):
        raise Rechazo(f"repo prohibido para Cursor: {repo}")
    if not _git(repo, "rev-parse", "--verify", "-q", "HEAD", check=False).strip():
        raise Rechazo(f"el repo no tiene commits: {repo}")
    vs = Path(cursor_argv()[1]).parent.name   # sin cursor-agent instalado falla aca, antes de crear nada
    run_id = time.strftime("%Y%m%d-%H%M%S-") + uuid.uuid4().hex[:6]
    (runs_dir() / run_id).mkdir(parents=True, exist_ok=True)
    (runs_dir() / run_id / "pedido.md").write_text(pedido, encoding="utf-8")
    wt, base, sembrados = _abrir_worktree(repo, run_id)
    r: dict[str, Any] = {"run_id": run_id, "repo": str(repo), "wt": str(wt), "base": base, "modelo": modelo,
                         "cli_version": vs, "correcciones": 0, "sembrados_reales": sembrados}
    try:
        _trabajar(r, f"{pedido.strip()}\n\n{REGLAS}")
    except BaseException as e:   # el resumen queda en disco: `descartar` siempre puede limpiar
        r.setdefault("estado", "error_tecnico")
        r.setdefault("error", f"{type(e).__name__}: {e}"[-600:])
        _guardar(r, "correr")
        raise
    _guardar(r, "correr")
    try:
        canal.post("cursor", "handoff", f"corrida {run_id} ({r['estado']}): {len(r['archivos'])} archivos, "
                   f"{len(r['bloqueadas'])} acciones bloqueadas", artifacts=[r["diff"]], to="claude")
    except Exception:  # ponytail: el canal es informativo
        pass
    return r


def corregir(run_id: str, pedido: str) -> dict:
    r = _cargar(run_id)
    if r.get("estado") == "alerta":
        raise Rechazo(f"ALERTA: cambiaron archivos sensibles: {r['alerta']}; descarta la corrida")
    if r.get("correcciones", 0) >= 1:
        raise Rechazo("una sola vuelta de correccion (ticket 05): termina Claude o descarta")
    if r.get("estado") not in ("listo", "error_tecnico") or not r.get("session_id"):
        raise Rechazo(f"la corrida esta {r.get('estado')} y no tiene sesion para retomar")
    try:
        _trabajar(r, f"CORRECCION DE CLAUDE:\n{pedido.strip()}\n\n{REGLAS}", resume=r.get("session_id"))
        r["correcciones"] = 1   # una falla tecnica no consume la correccion (ticket 05)
    finally:
        _guardar(r, "corregir")
    return r


def aplicar(run_id: str, permitir_borrados: bool = False, permitir_protegidos: bool = False, motivo: str = "") -> dict:
    r = _cargar(run_id)
    if r.get("estado") == "alerta":
        raise Rechazo(f"ALERTA: cambiaron archivos sensibles durante la corrida: {r['alerta']}")
    if r.get("estado") != "listo":
        raise Rechazo(f"la corrida esta {r.get('estado')}")
    if r["borrados"] and not permitir_borrados:
        raise Rechazo(f"el diff borra archivos (pedi permiso al usuario): {r['borrados']}")
    if r["protegidos"] and not permitir_protegidos:
        raise Rechazo(f"el diff toca credenciales o config de agentes (pedi permiso): {r['protegidos']}")
    repo = Path(r["repo"])
    patch = Path(r["diff"]).read_bytes()
    if patch.strip():
        _git_bytes(repo, "apply", "--whitespace=nowarn", "-", stdin=patch)
    _cerrar_worktree(repo, Path(r["wt"]))
    r["estado"] = "aplicado"
    _guardar(r, "aplicar", motivo=motivo, permitir_borrados=permitir_borrados, permitir_protegidos=permitir_protegidos)
    return r


def descartar(run_id: str, motivo: str = "") -> dict:
    r = _cargar(run_id)
    if r.get("estado") == "aplicado":
        raise Rechazo("la corrida ya se aplico: revertila en el repo si hace falta")
    _cerrar_worktree(Path(r["repo"]), Path(r["wt"]))
    r["estado"] = "descartado"
    _guardar(r, "descartar", motivo=motivo)
    return r


def _informe(r: dict) -> str:
    lineas = [f"corrida {r['run_id']}: {r.get('estado')} ({r.get('modelo_usado', r.get('modelo'))}, "
              f"{r.get('intentos')} intento(s), {r.get('secs')} s, {r.get('tokens')} tokens)",
              f"worktree: {r.get('wt')}", f"diff: {r.get('diff')}",
              f"archivos: {', '.join(r.get('archivos') or []) or 'ninguno'}"]
    for k, t in (("alerta", "ALERTA archivos sensibles cambiados"), ("error", "falla tecnica"),
                 ("borrados", "borra (pide permiso)"), ("protegidos", "toca protegidos (pide permiso)"),
                 ("bloqueadas", "acciones bloqueadas"),
                 ("cambios_en_repo", "cambios en el repo real durante la corrida (revisar)")):
        if r.get(k):
            lineas.append(f"{t}: {r[k]}")
    return "\n".join(lineas + ["", r.get("resultado", "")])


def main(argv: list[str] | None = None) -> int:
    a = list(sys.argv[1:] if argv is None else argv)
    for s in (sys.stdout, sys.stderr):
        getattr(s, "reconfigure", lambda **k: None)(encoding="utf-8", errors="replace")

    def flag(f: str) -> bool:
        if f in a:
            a.remove(f)
            return True
        return False
    try:
        if a[:1] == ["correr"] and len(a) >= 3:
            modelo = a[a.index("--modelo") + 1] if "--modelo" in a else MODELO
            r = correr(a[1], Path(a[2]).read_text(encoding="utf-8"), modelo)
        elif a[:1] == ["corregir"] and len(a) >= 3:
            r = corregir(a[1], Path(a[2]).read_text(encoding="utf-8"))
        elif a[:1] == ["aplicar"] and len(a) >= 2:
            borr, prot = flag("--permitir-borrados"), flag("--permitir-protegidos")
            r = aplicar(a[1], permitir_borrados=borr, permitir_protegidos=prot, motivo=" ".join(a[2:]))
        elif a[:1] == ["descartar"] and len(a) >= 2:
            r = descartar(a[1], " ".join(a[2:]))
        else:
            print(__doc__)
            return 1
    except Rechazo as e:
        print(f"RECHAZO: {e}")
        return 2
    print(_informe(r))
    return 0


if __name__ == "__main__":
    sys.exit(main())
