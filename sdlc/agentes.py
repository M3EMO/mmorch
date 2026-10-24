"""agentes — AGENTS.md fuente y CLAUDE.md con @AGENTS.md: append de bd sin pisar bytes y lint del contrato (orchestration-ior).

    python -m sdlc bd <repo>        # reglas y hooks de bd por append; exit 2 = no escribio nada, 1 = fallo un chequeo
    python -m sdlc lint-md <repo>   # contrato de docs/agents/estructura-md.md; exit 1 con hallazgos

Decisiones del grilling en brainstorms/2026-10-23-estructura-md-y-append-bd.md (D1..D11).
"""
from __future__ import annotations

import datetime
import difflib
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys

TPL_PROPIAS = pathlib.Path(__file__).resolve().parent / "templates" / "agentes" / "bd-propias.md"
PROPIAS = "## Reglas propias de bd"
IMPORT = "@AGENTS.md"
# bloque que escribe `bd setup claude`; bd marca hash = sha256(cuerpo.strip())[:8] y solo refresca un hash viejo
BLOQUE = re.compile(r"<!-- BEGIN BEADS INTEGRATION[^\n]*?hash:(\w+) -->\n(.*?)<!-- END BEADS INTEGRATION -->\n?", re.S)

# Hooks que administra bd (D9): bd los cambia o los borra; todo lo demas debe quedar igual.
BD_CMD = re.compile(r"^bd\s")  # comando de un hook de Claude en .claude/settings.json
BD_GIT = re.compile(r"# --- BEGIN BEADS INTEGRATION.*?# --- END BEADS INTEGRATION[^\n]*\n?", re.S)  # en .git/hooks

TOPES = {"AGENTS.md": 100, "CLAUDE.md": 60}  # lineas no vacias fuera del bloque de bd (D11)
ITEM_MAX = 200
SECCIONES = ["## Comandos", "## Reglas", "## Hooks", "## Mapa"]  # orden fijo en AGENTS.md (D6)


class Rechazo(Exception):
    def __init__(self, codigo: int, msg: str):
        super().__init__(msg)
        self.codigo = codigo


def _hash(cuerpo: str) -> str:
    return hashlib.sha256(cuerpo.strip().encode()).hexdigest()[:8]


def _norma(linea: str) -> str:
    return " ".join(linea.split())


def _git(root: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, encoding="utf-8")


def _bd(root: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    r = subprocess.run(["bd", *args], cwd=root, capture_output=True, text=True, encoding="utf-8")
    if r.returncode and args[-1] != "--check":
        raise Rechazo(1, f"bd {' '.join(args)} fallo: {(r.stderr or r.stdout).strip()[-400:]}")
    return r


def _original(root: pathlib.Path, marca: str) -> str | None:
    """Cuerpo del bloque tal como lo escribio bd: la revision git cuyo cuerpo da el hash de su marcador (D4b)."""
    revs = _git(root, "log", "--all", "--format=%H", f"-Shash:{marca}", "--", "CLAUDE.md", "AGENTS.md").stdout.split()
    for rev in [r for c in revs for r in (c, c + "^")]:
        for nombre in ("CLAUDE.md", "AGENTS.md"):
            for m in BLOQUE.finditer(_git(root, "show", f"{rev}:{nombre}").stdout):
                if m.group(1) == marca and _hash(m.group(2)) == marca:
                    return m.group(2)
    return None


def _restaurar(root: pathlib.Path, texto: str, propias: dict[str, list[str]]) -> str:
    """Devuelve cada bloque editado a su original y junta en `propias` las lineas agregadas o cambiadas."""
    partes, fin = [], 0
    for m in BLOQUE.finditer(texto):
        marca, cuerpo = m.group(1), m.group(2)
        nuevo = cuerpo
        if _hash(cuerpo) != marca:
            nuevo = _original(root, marca)
            if nuevo is None:
                raise Rechazo(2, f"bloque de bd editado (hash {marca}) sin original en git; no escribo nada")
            a, b = nuevo.splitlines(keepends=True), cuerpo.splitlines(keepends=True)
            de_bd = {_norma(x) for x in a}  # un cambio que solo toca espacios no es una regla propia
            for op, _, _, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
                if op in ("replace", "insert"):
                    propias.setdefault(marca, []).extend(x for x in b[j1:j2] if x.strip() and _norma(x) not in de_bd)
        partes += [texto[fin:m.start(2)], nuevo]
        fin = m.end(2)
    return "".join(partes) + texto[fin:]


def _append(texto: str, extra: str) -> str:
    """Agrega `extra` al final con una linea en blanco de separacion, sin tocar un byte previo."""
    if not texto:
        return extra
    return texto + ("" if texto.endswith("\n") else "\n") + "\n" + extra


def sin_bd(settings: dict) -> dict:
    """settings.json sin los hooks de bd: el oraculo de D9 exige que esto no cambie."""
    out = json.loads(json.dumps(settings))
    hooks = out.get("hooks", {})
    for evento in list(hooks):
        grupos = []
        for g in hooks[evento]:
            hs = [h for h in g.get("hooks", []) if not BD_CMD.match(h.get("command", ""))]
            if hs or not g.get("hooks"):
                grupos.append({**g, "hooks": hs})
        if grupos:
            hooks[evento] = grupos
        else:
            del hooks[evento]
    if "hooks" in out and not hooks:
        del out["hooks"]
    return out


def git_hook_sin_bd(texto: str) -> str:
    return BD_GIT.sub("", texto).strip()


def _git_hooks(root: pathlib.Path) -> pathlib.Path:
    return (root / _git(root, "rev-parse", "--git-path", "hooks").stdout.strip()).resolve()


def bd_append(repo: str, hoy: str | None = None) -> dict:
    root = pathlib.Path(repo).resolve()
    if shutil.which("bd") is None:
        raise Rechazo(2, "falta el binario bd")
    if _git(root, "rev-parse", "--git-dir").returncode:
        raise Rechazo(2, f"{root} no es un repo git")
    claude, agents = root / "CLAUDE.md", root / "AGENTS.md"
    antes = {p: p.read_text(encoding="utf-8") if p.exists() else "" for p in (claude, agents)}

    # fase 1, sin escribir: un bloque editado sin original corta aca
    propias: dict[str, list[str]] = {}
    t_claude = _restaurar(root, antes[claude], propias)
    t_agents = _restaurar(root, antes[agents], propias)

    hecho: list[str] = []
    if not (root / ".beads").exists():
        head = _git(root, "rev-parse", "--verify", "-q", "HEAD").stdout.strip()
        _bd(root, "init", "--skip-agents", "--skip-hooks", "--init-if-missing")
        # bd init commitea si git tiene identidad y no hay flag que lo evite; D10 = en stage, sin commit
        if _git(root, "rev-parse", "--verify", "-q", "HEAD").stdout.strip() != head:
            _git(root, *(["reset", "--soft", head] if head else ["update-ref", "-d", "HEAD"]))
        hecho.append("bd init")
    t_claude = BLOQUE.sub("", t_claude)  # el bloque de bd vive en AGENTS.md (D1); aca ya es texto puro de bd
    if IMPORT not in t_claude.splitlines():
        t_claude = _append(t_claude, IMPORT + "\n")
    t_agents = t_agents or f"# {root.name}\n"
    for p, t in ((claude, t_claude), (agents, t_agents)):
        if t != antes[p]:
            p.write_text(t, encoding="utf-8")
            hecho.append(p.name)

    settings = root / ".claude" / "settings.json"
    s_antes = settings.read_bytes() if settings.exists() else None
    _bd(root, "setup", "claude")
    if s_antes is not None and sin_bd(json.loads(s_antes)) != sin_bd(json.loads(settings.read_text(encoding="utf-8"))):
        settings.write_bytes(s_antes)
        raise Rechazo(1, "bd setup claude cambio hooks que no son de bd; settings.json restaurado")
    if claude.read_text(encoding="utf-8") != t_claude:
        raise Rechazo(1, "bd setup claude escribio en CLAUDE.md pese a @AGENTS.md")
    # las reglas propias van despues del bloque de bd (D6): se agregan sobre lo que dejo bd setup
    t_agents = con_bd = agents.read_text(encoding="utf-8")
    if PROPIAS not in t_agents:
        t_agents = _append(t_agents, TPL_PROPIAS.read_text(encoding="utf-8"))
    vistas = {_norma(x) for x in t_agents.splitlines()}
    escritas: dict[str, int] = {}
    for marca, lineas in propias.items():
        nuevas = []
        for x in lineas:
            if _norma(x) not in vistas:
                nuevas.append(x)
                vistas.add(_norma(x))
        escritas[marca] = len(nuevas)
        if nuevas:
            fecha = hoy or datetime.date.today().isoformat()
            t_agents = _append(t_agents, f"{PROPIAS} — reintegradas del bloque {marca} ({fecha})\n\n" + "".join(nuevas))
    if t_agents != con_bd:
        agents.write_text(t_agents, encoding="utf-8")
        hecho += [] if agents.name in hecho else [agents.name]

    dir_hooks = _git_hooks(root)
    previos = {p.name: git_hook_sin_bd(p.read_text(encoding="utf-8")) for p in dir_hooks.glob("*")
               if p.is_file() and not p.name.endswith((".sample", ".backup"))}
    _bd(root, "hooks", "install")
    for nombre, propio in previos.items():
        if propio not in (dir_hooks / nombre).read_text(encoding="utf-8"):
            raise Rechazo(1, f"bd hooks install perdio lineas propias de .git/hooks/{nombre}")

    check = _bd(root, "setup", "claude", "--check")
    if check.returncode:
        raise Rechazo(1, f"bd setup claude --check fallo: {check.stdout.strip()[-400:]}")
    return {"repo": str(root), "hecho": hecho, "reintegradas": escritas}


def _es_contrato(linea: str) -> bool:
    s = linea.lstrip()
    return (s.startswith(("#", "- ", "* ", "|", "<!--", "@", "North-star:")) or re.match(r"\d+\. ", s) is not None)


def _seccion(texto: str, titulo: str) -> str:
    m = re.search(rf"^{re.escape(titulo)}\s*\n(.*?)(?=^## |\Z)", texto, re.S | re.M)
    return m.group(1) if m else ""


def lint(repo: str) -> list[str]:
    root = pathlib.Path(repo).resolve()
    hallazgos: list[str] = []
    textos: dict[str, str] = {}
    for nombre, tope in TOPES.items():
        p = root / nombre
        if not p.exists():
            hallazgos.append(f"{nombre}: falta")
            continue
        textos[nombre] = t = BLOQUE.sub("", p.read_text(encoding="utf-8"))
        en_codigo, cuenta = False, 0
        for i, linea in enumerate(t.splitlines(), 1):
            if linea.startswith("```"):
                en_codigo = not en_codigo
            if not linea.strip():
                continue
            cuenta += 1
            if len(linea) > ITEM_MAX:
                hallazgos.append(f"{nombre}:{i}: linea de {len(linea)} caracteres (tope {ITEM_MAX})")
            if not en_codigo and not linea.startswith("```") and not _es_contrato(linea):
                hallazgos.append(f"{nombre}:{i}: parrafo; usa un item de una linea o llevalo a docs/agents/")
        if cuenta > tope:
            hallazgos.append(f"{nombre}: {cuenta} lineas propias (tope {tope})")
    if "CLAUDE.md" in textos and IMPORT not in textos["CLAUDE.md"].splitlines():
        hallazgos.append(f"CLAUDE.md: falta la linea {IMPORT}")
    ta = textos.get("AGENTS.md")
    if ta is not None:
        lineas = [x for x in ta.splitlines() if x.strip()]
        if not lineas or not lineas[0].startswith("# "):
            hallazgos.append("AGENTS.md: la primera linea no es el titulo `# <repo>`")
        if not any(x.startswith("North-star:") for x in lineas):
            hallazgos.append("AGENTS.md: falta la linea `North-star:`")
        pos = [lineas.index(s) if s in lineas else -1 for s in SECCIONES]
        for s, i in zip(SECCIONES, pos, strict=True):
            if i < 0:
                hallazgos.append(f"AGENTS.md: falta `{s}`")
        if [i for i in pos if i >= 0] != sorted(i for i in pos if i >= 0):
            hallazgos.append(f"AGENTS.md: secciones fuera de orden; orden: {', '.join(SECCIONES)}")
        hooks = _seccion(ta, "## Hooks")
        settings = root / ".claude" / "settings.json"
        cmds = [h.get("command", "") for gs in (json.loads(settings.read_text(encoding="utf-8")).get("hooks", {}).values()
                                                if settings.exists() else []) for g in gs for h in g.get("hooks", [])]
        if _git(root, "rev-parse", "--git-dir").returncode == 0:
            cmds += [p.name for p in _git_hooks(root).glob("*")
                     if p.is_file() and not p.name.endswith((".sample", ".backup"))]
        for c in dict.fromkeys(cmds):
            if c and c not in hooks:
                hallazgos.append(f"AGENTS.md: `## Hooks` no nombra el hook `{c}`")
    return hallazgos


def main(argv: list[str]) -> int:
    if len(argv) not in (1, 2) or argv[0] not in ("bd", "lint-md"):
        print("uso: python -m sdlc bd <repo> | python -m sdlc lint-md <repo>", file=sys.stderr)
        return 2
    repo = argv[1] if len(argv) > 1 else "."
    if argv[0] == "lint-md":
        hallazgos = lint(repo)
        print("\n".join(hallazgos) or "ok")
        return 1 if hallazgos else 0
    try:
        print(json.dumps(bd_append(repo), ensure_ascii=False))
    except Rechazo as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return e.codigo
    return 0
