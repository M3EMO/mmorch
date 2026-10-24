"""convenciones — los repos propios convergen a las costumbres vigentes (mapa `.scratch/repos-convenciones/`).

    python -m sdlc doctor <repo> | --todos [raiz]   # oraculo: reporte JSON, nunca escribe ni llama a un LLM
    python -m sdlc init --upgrade <repo>            # aplica solo arreglos seguros; commitea sus rutas y pushea
    python -m sdlc areas <repo> [--aprobar]         # propuesta de areas (carpetas + ajustes de un LLM barato)
    python -m sdlc propiedades <repo> [--aprobar]   # Hypothesis / fast-check en el repo

Exit de doctor: 0 sin hallazgos, 3 solo avisos, 1 con desvios, 4 sin sdlc.toml (no adoptado), 2 uso.
Exit de los que escriben: 0 hecho (o nada que hacer), 1 abortado o fallido, 4 sin sdlc.toml, 2 uso.
`--upgrade`, `areas --aprobar` y `propiedades --aprobar` pushean: un agente pide OK antes (push = tier ASK).
Decisiones y tests aprobados: `## Answer` de cada ticket en `.scratch/repos-convenciones/issues/`.
"""
from __future__ import annotations

import dataclasses
import difflib
import fnmatch
import json
import os
import pathlib
import re
import shlex
import subprocess
import sys
import tomllib
from collections.abc import Callable

from . import pipeline as P
from .agentes import IMPORT, _append, _hash, _leer

PUSH_TIMEOUT_S = 120
PUBLICADAS = json.loads((pathlib.Path(__file__).with_name("plantillas_publicadas.json")).read_text(encoding="utf-8"))
NOMBRE = re.compile(r"[a-z0-9_-]+$")
PUNTERO_LEGACY = "## SDLC (pipeline de 6 etapas)"
PUNTERO_V1 = (f"{PUNTERO_LEGACY}\n\nEste repo construye features con el paquete `sdlc` (skill `/project`). Contrato del "
              "repo en `sdlc.toml`; artefactos de cada corrida en `docs/sdlc/` de la review branch.\n")
# bloques que este mecanismo es dueño: nombre -> {version: interior}; una version nueva se agrega, nunca se borra
BLOQUES: dict[str, dict[int, str]] = {"puntero": {1: PUNTERO_V1},
                                      "propiedades": {1: "RUN npm install --prefix / fast-check\n"}}
SECRETOS = {".env": ".env", "credentials.json": "credentials.json", "token.json": "token.json", "*.key": "x.key"}
YAML_OK = (".pre-commit-config.yaml", ".beads/*.yaml", ".github/workflows/*.yml", ".github/workflows/*.yaml",
           "docker-compose.yml")
# claves de primer nivel que algun lector conoce (sdlc/pipeline.py, sprites, areas); otra clave = aviso
CLAVES = {"accept_cmd", "suite", "files", "ext", "approve_accept", "usd_max", "compile_cmd", "lint_cmd", "suite_cmd",
          "seed_globs", "accept_test", "constitution", "cobertura_min", "mutation_min", "mutation_muestra", "reviewer_cmd",
          "contenedor", "python", "tamano_max_archivos", "tamano_max_tokens", "sprites", "stall_rounds",
          "diff_novelty_min", "cmd_timeout_s", "suite_timeout_s", "planner_timeout_s", "drift_max", "cierra_loop", "rama",
          "areas", "spec_de_area"}
ABSOLUTA = re.compile(r"^(?:[A-Za-z]:[\\/]|/|~|\\\\)")


class Rechazo(Exception):
    def __init__(self, codigo: int, msg: str):
        super().__init__(msg)
        self.codigo = codigo


@dataclasses.dataclass
class H:
    """Un hallazgo de `doctor`. `fix` = arreglo seguro (texto viejo -> texto nuevo de `ruta`); None = solo reporte."""
    costumbre: str
    nivel: str  # "desvio" | "aviso"
    ruta: str
    detalle: str
    fix: Callable[[str], str] | None = None
    completo: bool = True  # False: el arreglo ayuda pero no cierra el desvio (clave sin valor adivinable)

    def json(self) -> dict:
        return {"costumbre": self.costumbre, "nivel": self.nivel, "ruta": self.ruta, "detalle": self.detalle,
                "seguro": self.fix is not None}

    def clave(self) -> tuple:
        return (self.costumbre, self.ruta, self.detalle)


@dataclasses.dataclass
class Ctx:
    root: pathlib.Path
    texto: str  # sdlc.toml con saltos LF
    cfg: dict | None  # None = sdlc.toml roto
    error: str
    archivos: list[str]  # git ls-files
    es_git: bool


# --------------------------------------------------------------------------------------------------- utilidades

def _git(root: pathlib.Path, *args: str, **kw) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, encoding="utf-8", **kw)


def _ctx(root: pathlib.Path, cfg_extra: dict | None = None) -> Ctx:
    texto = _leer(root / "sdlc.toml")[0]
    try:
        cfg, error = tomllib.loads(texto), ""
    except tomllib.TOMLDecodeError as e:
        cfg, error = None, str(e)
    if cfg is not None and cfg_extra:
        cfg = {**cfg, **cfg_extra}
    ls = _git(root, "ls-files")
    return Ctx(root, texto, cfg, error, sorted(ls.stdout.splitlines()) if ls.returncode == 0 else [], ls.returncode == 0)


def _atomico(p: pathlib.Path, data: bytes | None) -> None:
    """Escritura atomica (tmp en la misma carpeta + os.replace); None borra el archivo."""
    if data is None:
        p.unlink(missing_ok=True)
        return
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_name(f".{p.name}.sdlc-tmp-{os.getpid()}")
    try:
        tmp.write_bytes(data)
        os.replace(tmp, p)
    finally:
        tmp.unlink(missing_ok=True)


def _bytes(texto: str, eol: str) -> bytes:
    return texto.replace("\n", eol).encode("utf-8")


def _fin(texto: str) -> str:
    return texto if not texto or texto.endswith("\n") else texto + "\n"


def _antes_de_tablas(texto: str, linea: str) -> str:
    """Inserta una clave suelta antes del primer encabezado de tabla: al final caeria dentro de `[sprites]`."""
    m = re.search(r"^\[", texto, re.M)
    if not m:
        return _fin(texto) + linea + "\n"
    return texto[:m.start()] + linea + "\n" + texto[m.start():]


def _const(texto: str) -> Callable[[str], str]:
    return lambda _t: texto


def _sumar_lineas(lineas: list[str]) -> Callable[[str], str]:
    return lambda t: _fin(t) + "".join(x + "\n" for x in lineas)


def _diff(viejo: str, nuevo: str, ruta: str) -> str:
    return "".join(difflib.unified_diff(viejo.splitlines(True), nuevo.splitlines(True), f"{ruta} (repo)",
                                        f"{ruta} (vigente)"))


# --------------------------------------------------------------------------------------------------- bloques

def _estilo(ruta: str) -> tuple[str, str]:
    return ("# ", "") if pathlib.PurePath(ruta).name.startswith("Dockerfile") else ("<!-- ", " -->")


def bloque(nombre: str, ruta: str, v: int | None = None) -> str:
    v = v or max(BLOQUES[nombre])
    a, b = _estilo(ruta)
    interior = BLOQUES[nombre][v]
    return f"{a}BEGIN SDLC {nombre} v:{v} hash:{_hash(interior)}{b}\n{interior}{a}END SDLC {nombre}{b}\n"


def puntero_marcado() -> str:
    return bloque("puntero", "AGENTS.md")


def _bloques(texto: str, nombre: str, ruta: str) -> tuple[list[re.Match], str | None]:
    a, b = (re.escape(x) for x in _estilo(ruta))
    rx = re.compile(rf"^{a}BEGIN SDLC {re.escape(nombre)} v:(\d+) hash:(\w+){b}\n(.*?)^{a}END SDLC {re.escape(nombre)}{b}\n?",
                    re.S | re.M)
    hallados = list(rx.finditer(texto))
    abre = [m.start() for m in re.finditer(rf"^{a}BEGIN SDLC {re.escape(nombre)}\b", texto, re.M)]
    cierra = len(re.findall(rf"^{a}END SDLC {re.escape(nombre)}{b}$", texto, re.M))
    if len(abre) != len(hallados) or cierra != len(hallados) or len(hallados) > 1:
        linea = texto[:abre[0]].count("\n") + 1 if abre else texto.count("\n") + 1
        return hallados, f"marca SDLC {nombre} rota o duplicada (linea {linea})"
    return hallados, None


def _chequeo_bloque(texto: str, nombre: str, ruta: str, costumbre: str, requerido: bool) -> list[H]:
    hallados, rota = _bloques(texto, nombre, ruta)
    if rota:
        return [H(costumbre, "aviso", ruta, rota)]
    vigente = max(BLOQUES[nombre])
    if not hallados:
        if not requerido:
            return []
        return [H(costumbre, "desvio", ruta, f"falta el bloque {nombre}", lambda t: _append(t, bloque(nombre, ruta)))]
    m = hallados[0]
    v, marca, interior = int(m.group(1)), m.group(2), m.group(3)
    if _hash(interior) != marca:
        return [H(costumbre, "aviso", ruta, f"bloque {nombre} editado a mano\n"
                  + _diff(interior, BLOQUES[nombre][vigente], ruta))]
    if v > vigente:
        return [H(costumbre, "aviso", ruta, f"bloque {nombre} v:{v} mas nuevo que esta copia de sdlc (v:{vigente})")]
    if v < vigente:
        return [H(costumbre, "desvio", ruta, f"bloque {nombre} v:{v} viejo (vigente v:{vigente})",
                  lambda t: t.replace(m.group(0), bloque(nombre, ruta), 1))]
    return []


# --------------------------------------------------------------------------------------------------- chequeos

def _c_toml(c: Ctx) -> list[H]:
    if c.cfg is None:
        return [H("toml", "desvio", "sdlc.toml", f"sdlc.toml roto: {c.error}")]
    out = []
    if "files" not in c.cfg:
        globs = P._globs_iniciales(c.root)[1]
        linea = "files = [\n" + "".join(f'    "{g}",\n' for g in globs) + "]"
        out.append(H("toml", "desvio", "sdlc.toml", "falta la clave files", lambda t: _antes_de_tablas(t, linea)))
    if "accept_cmd" not in c.cfg:
        if (c.root / "tests").is_dir() and "py" in (c.cfg.get("ext") or ["py"]):
            out.append(H("toml", "desvio", "sdlc.toml", "falta la clave accept_cmd",
                         lambda t: _antes_de_tablas(t, 'accept_cmd = "python -m pytest -q"')))
        elif "# accept_cmd =" not in c.texto:
            out.append(H("toml", "desvio", "sdlc.toml", "falta la clave accept_cmd", completo=False, fix=lambda t:
                         _antes_de_tablas(t, '# accept_cmd = "<comando determinista, sin red>"   # sin esto el repo no entra')))
        else:
            out.append(H("toml", "desvio", "sdlc.toml", "falta la clave accept_cmd"))
    out += [H("toml-clave", "aviso", "sdlc.toml", f"clave desconocida: {k}") for k in sorted(set(c.cfg) - CLAVES)]
    return out


def _textos(valor, camino: str):
    if isinstance(valor, str):
        yield camino, valor
    elif isinstance(valor, list):
        for i, x in enumerate(valor):
            yield from _textos(x, f"{camino}[{i}]")
    elif isinstance(valor, dict):
        for k, x in valor.items():
            yield from _textos(x, f"{camino}.{k}" if camino else k)


def _ruta_absoluta(texto: str) -> str | None:
    try:
        piezas = shlex.split(texto)
    except ValueError:
        piezas = []
    for p in piezas + texto.split():  # shlex come las barras de una ruta de red `\\srv`; split las conserva
        for q in [p] + ([p.split("=", 1)[1]] if p.startswith("-") and "=" in p else []):
            if ABSOLUTA.match(q):
                return q
    return None


def _c_rutas(c: Ctx) -> list[H]:
    if c.cfg is None:
        return []
    return [H("rutas-absolutas", "desvio", "sdlc.toml", f"{clave}: ruta de una sola maquina: {r}")
            for clave, texto in _textos(c.cfg, "") if (r := _ruta_absoluta(texto))]


def _c_legacy(c: Ctx) -> list[H]:
    if (c.root / "docs" / "sdlc" / "sdlc.toml").exists():
        return [H("toml-legacy", "desvio", "docs/sdlc/sdlc.toml", "costumbre retirada: el pipeline ya no lo lee; "
                  "pasa sus claves al sdlc.toml de la raiz y borralo")]
    return []


def _c_plantillas(c: Ctx) -> list[H]:
    out = []
    for t in sorted(P.TPL.glob("*.md")):
        ruta = f"docs/sdlc/{t.name}"
        vigente = t.read_text(encoding="utf-8")
        p = c.root / ruta
        if not p.exists():
            out.append(H("plantillas", "desvio", ruta, "falta la plantilla", _const(vigente)))
            continue
        actual = _leer(p)[0]
        h = _hash(actual)
        if h == _hash(vigente):
            continue
        if h in PUBLICADAS.get(t.name, []):
            out.append(H("plantillas", "desvio", ruta, "plantilla vieja e intacta", _const(vigente)))
        else:
            out.append(H("plantillas", "aviso", ruta, "plantilla editada a mano\n" + _diff(actual, vigente, ruta)))
    return out


def _c_agents(c: Ctx) -> list[H]:
    p = c.root / "AGENTS.md"
    if not p.exists():
        return [H("agents", "desvio", "AGENTS.md", "falta AGENTS.md",
                  lambda _t: f"# {c.root.name}\n\n" + puntero_marcado())]
    texto = _leer(p)[0]
    hallados, _ = _bloques(texto, "puntero", "AGENTS.md")
    sin_bloques = re.sub(r"<!-- BEGIN SDLC puntero .*?<!-- END SDLC puntero -->", "", texto, flags=re.S)
    if not hallados and PUNTERO_LEGACY in sin_bloques:
        return [H("agents", "aviso", "AGENTS.md", "puntero del SDLC sin marca de cierre; no se reescribe")]
    return _chequeo_bloque(texto, "puntero", "AGENTS.md", "agents", requerido=True)


def _c_claude(c: Ctx) -> list[H]:
    p = c.root / "CLAUDE.md"
    if p.exists() and IMPORT not in _leer(p)[0].splitlines():
        return [H("claude-import", "desvio", "CLAUDE.md", f"falta la linea {IMPORT}",
                  lambda t: IMPORT + "\n" + ("\n" + t if t else ""))]
    return []


def _c_gitignore(c: Ctx) -> list[H]:
    if not c.es_git:
        return []
    faltan = [pat for pat, muestra in SECRETOS.items()
              if _git(c.root, "check-ignore", "-q", "--no-index", muestra).returncode != 0]
    out = []
    if faltan:
        out.append(H("gitignore", "desvio", ".gitignore", f"patrones de secretos sin ignorar: {', '.join(faltan)}",
                     _sumar_lineas(faltan)))
    for f in c.archivos:
        nombre = pathlib.PurePath(f).name
        if nombre in (".env", "credentials.json", "token.json") or nombre.endswith(".key"):
            out.append(H("gitignore", "desvio", f, "secreto trackeado: sacalo del indice y rota la credencial; "
                         "el historial lo decide el usuario"))
    return out


def _c_beads(c: Ctx) -> list[H]:
    if not (c.root / ".beads").is_dir():
        return [H("beads", "desvio", ".beads", "falta .beads/; arreglo: python -m sdlc bd <repo> (orchestration-ior)")]
    return []


def _c_rama(c: Ctx) -> list[H]:
    if not c.es_git or _git(c.root, "rev-parse", "--abbrev-ref", "@{u}").returncode:
        return []
    n = _git(c.root, "rev-list", "--count", "HEAD..@{u}").stdout.strip()
    if n and n != "0":
        return [H("rama", "aviso", ".", f"la rama local esta {n} commits detras de su remoto (refs locales, sin fetch)")]
    return []


def _de_area(c: Ctx, globs: list[str]) -> set[str]:
    return {f for f in c.archivos if any(P._calza(f, g) for g in globs)}


def _c_areas(c: Ctx) -> list[H]:
    areas = (c.cfg or {}).get("areas")
    if areas is None:
        return []
    if not isinstance(areas, dict):
        return [H("areas", "desvio", "sdlc.toml", "[areas] debe ser una tabla nombre = [globs]")]
    out: list[H] = []
    por_archivo: dict[str, list[str]] = {}
    for nombre, globs in sorted(areas.items()):
        if not NOMBRE.match(nombre):
            out.append(H("areas", "desvio", "sdlc.toml", f"area {nombre}: el nombre no cumple [a-z0-9_-]+"))
        if not (isinstance(globs, list) and globs and all(isinstance(g, str) and g for g in globs)):
            out.append(H("areas", "desvio", "sdlc.toml", f"area {nombre}: necesita una lista de globs"))
            continue
        archivos = _de_area(c, globs)
        if not archivos:
            out.append(H("areas", "desvio", "sdlc.toml", f"area {nombre}: ningun archivo trackeado calza"))
        for f in archivos:
            por_archivo.setdefault(f, []).append(nombre)
    out += [H("areas", "desvio", f, f"el archivo calza con dos areas o mas: {', '.join(sorted(ns))}")
            for f, ns in sorted(por_archivo.items()) if len(ns) > 1]
    return out


def _c_spec_de_area(c: Ctx) -> list[H]:
    cfg = c.cfg or {}
    specs, areas = cfg.get("spec_de_area") or {}, cfg.get("areas") or {}
    if not isinstance(specs, dict) or not isinstance(areas, dict):
        return [H("spec-de-area", "desvio", "sdlc.toml", "[spec_de_area] debe ser una tabla nombre = \"ruta\"")] \
            if not isinstance(specs, dict) else []
    out: list[H] = []
    rutas: dict[str, list[str]] = {}
    for nombre in sorted(set(areas) | set(specs)):
        ruta = spec_base(cfg, nombre)
        if nombre in specs:
            if nombre not in areas:
                out.append(H("spec-de-area", "desvio", "sdlc.toml", f"spec_de_area.{nombre}: el area no existe en [areas]"))
            if not isinstance(ruta, str) or ABSOLUTA.match(ruta):
                continue  # la ruta absoluta ya la marca rutas-absolutas
            destino = (c.root / ruta).resolve()
            if not destino.is_relative_to(c.root):
                out.append(H("spec-de-area", "desvio", "sdlc.toml", f"spec_de_area.{nombre}: la ruta sale del repo: {ruta}"))
                continue
            if not destino.is_file():
                out.append(H("spec-de-area", "desvio", "sdlc.toml", f"spec_de_area.{nombre}: no existe {ruta}"))
        rutas.setdefault(os.path.normpath(ruta), []).append(nombre)
    out += [H("spec-de-area", "desvio", "sdlc.toml", f"dos areas apuntan a la misma spec {r}: {', '.join(ns)}")
            for r, ns in sorted(rutas.items()) if len(ns) > 1]
    return out


def spec_base(cfg: dict, area: str) -> str:
    """Spec viva de un area: `[spec_de_area]` si la declara, si no `docs/specs/<area>/spec.md` (ticket 06)."""
    return (cfg.get("spec_de_area") or {}).get(area, f"docs/specs/{area}/spec.md")


def _specs_planas(root: pathlib.Path) -> list[pathlib.Path]:
    d = root / "docs" / "specs"
    return sorted(p for p in d.glob("*.md") if p.is_file()) if d.is_dir() else []


def _c_sin_areas(c: Ctx) -> list[H]:
    if c.cfg is not None and "areas" not in c.cfg and _specs_planas(c.root):
        return [H("areas", "aviso", "docs/specs", "el repo tiene specs planas y no declara areas; "
                  "propuesta: python -m sdlc areas <repo>")]
    return []


def _c_constitucion(c: Ctx) -> list[H]:
    ruta = str((c.cfg or {}).get("constitution") or "GOAL.md")
    if not (c.root / ruta).is_file():
        return [H("constitucion", "aviso", ruta, "sin constitucion: spec y plan no leen reglas fijas del repo; "
                  "declara `constitution` en sdlc.toml si el repo las tiene")]
    return []


def _c_yaml(c: Ctx) -> list[H]:
    return [H("formato", "aviso", f, "YAML fuera de la lista de herramientas: la configuracion propia va en TOML y "
              "los datos entre programas en JSON") for f in c.archivos
            if f.endswith((".yml", ".yaml")) and not any(fnmatch.fnmatch(f, pat) for pat in YAML_OK)]


def _c_propiedades(c: Ctx) -> list[H]:
    if c.cfg is None:
        return []
    return [H("propiedades", "aviso", d["deps"] or ".", f"falta {d['lib']}: los tests de propiedades no corren; "
              "propuesta: python -m sdlc propiedades <repo>") for d in _necesarias(c) if not d["presente"]]


def _c_dockerfile(c: Ctx) -> list[H]:
    p = c.root / "Dockerfile.sdlc"
    return _chequeo_bloque(_leer(p)[0], "propiedades", "Dockerfile.sdlc", "dockerfile", requerido=False) \
        if p.exists() else []


# tabla unica de costumbres (ticket 01): una fila = id, grupo y su chequeo determinista sin red
COSTUMBRES: list[tuple[str, str, Callable[[Ctx], list[H]]]] = [
    ("toml", "sdlc", _c_toml),
    ("rutas-absolutas", "sdlc", _c_rutas),
    ("toml-legacy", "sdlc", _c_legacy),
    ("plantillas", "sdlc", _c_plantillas),
    ("agents", "sdlc", _c_agents),
    ("claude-import", "sdlc", _c_claude),
    ("rama", "sdlc", _c_rama),
    ("gitignore", "secretos", _c_gitignore),
    ("beads", "bd", _c_beads),
    ("areas", "sdd", _c_areas),
    ("spec-de-area", "sdd", _c_spec_de_area),
    ("sin-areas", "sdd", _c_sin_areas),
    ("constitucion", "sdd", _c_constitucion),
    ("formato", "formato", _c_yaml),
    ("propiedades", "sdd", _c_propiedades),
    ("dockerfile", "sdlc", _c_dockerfile),
]


def _diagnostico(root: pathlib.Path, cfg_extra: dict | None = None) -> list[H]:
    c = _ctx(root, cfg_extra)
    return sorted((h for _, _, f in COSTUMBRES for h in f(c)), key=lambda h: (h.nivel, h.clave()))


def doctor(repo: str) -> dict:
    """Reporte JSON del repo: desvios y avisos en listas separadas. Determinista: sin red, sin LLM, sin escrituras."""
    root = pathlib.Path(repo).resolve()
    if not (root / "sdlc.toml").exists():
        return {"repo": str(root), "adoptado": False, "desvios": [], "avisos": []}
    hs = _diagnostico(root)
    return {"repo": str(root), "adoptado": True, "desvios": [h.json() for h in hs if h.nivel == "desvio"],
            "avisos": [h.json() for h in hs if h.nivel == "aviso"]}


def codigo(rep: dict) -> int:
    return 4 if not rep["adoptado"] else 1 if rep["desvios"] else 3 if rep["avisos"] else 0


# --------------------------------------------------------------------------------------------------- --todos

def repos(raiz: str | None = None) -> list[str]:
    """Repos adoptados bajo `raiz` (default home): `.git` directorio + `sdlc.toml`. Salta worktrees (`.git` archivo),
    carpetas con punto, node_modules y carpetas sin permiso; no sigue symlinks."""
    base = pathlib.Path(raiz or pathlib.Path.home()).resolve()
    out = []
    for d, subdirs, archivos in os.walk(base, followlinks=False, onerror=lambda _e: None):
        subdirs[:] = sorted(s for s in subdirs if not s.startswith(".") and s != "node_modules")
        if "sdlc.toml" in archivos and os.path.isdir(os.path.join(d, ".git")):
            out.append(str(pathlib.Path(d)))
    return sorted(out)


# --------------------------------------------------------------------------------------------------- escritura

def rama_default(root: pathlib.Path, cfg: dict | None) -> str | None:
    if cfg and isinstance(cfg.get("rama"), str):
        return cfg["rama"]
    r = _git(root, "symbolic-ref", "--short", "refs/remotes/origin/HEAD")
    if r.returncode == 0 and r.stdout.strip():
        return r.stdout.strip().split("/", 1)[-1]
    locales = set(_git(root, "for-each-ref", "--format=%(refname:short)", "refs/heads").stdout.split())
    cand = locales & {"main", "master"}
    return cand.pop() if len(cand) == 1 else None


def _precheck(root: pathlib.Path, cfg: dict | None, rutas: list[str]) -> str:
    """Rama default y archivos limpios (ticket 07); lanza Rechazo(1) sin escribir nada."""
    rama = rama_default(root, cfg)
    actual = _git(root, "symbolic-ref", "--short", "-q", "HEAD").stdout.strip()
    if rama is None:
        raise Rechazo(1, "no se sabe la rama default: declara `rama` en sdlc.toml")
    if actual != rama:
        raise Rechazo(1, f"rama actual {actual or '(HEAD separado)'}; esperada {rama}")
    sucios = [r for r in rutas if _git(root, "status", "--porcelain", "--", r).stdout.strip()]
    if sucios:
        raise Rechazo(1, f"archivos sucios que habria que tocar: {', '.join(sucios)}")
    return rama


def _commit_push(root: pathlib.Path, rama: str, rutas: list[str], mensaje: str) -> dict:
    """Commitea solo `rutas` (lo staged del usuario queda staged y afuera) y pushea sin --force ni prompt."""
    _git(root, "add", "--", *rutas)
    c = _git(root, "commit", "-q", "-m", mensaje, "--", *rutas)
    if c.returncode:
        return {"commit": None, "error": f"commit fallo (cambios sin commitear): {(c.stderr or c.stdout).strip()[-400:]}"}
    rep = {"commit": _git(root, "rev-parse", "--short", "HEAD").stdout.strip(), "push": None}
    remoto = _git(root, "config", f"branch.{rama}.remote").stdout.strip() or \
        ("origin" if "origin" in _git(root, "remote").stdout.split() else "")
    if not remoto:
        rep["aviso"] = "sin remoto: el commit queda local"
        return rep
    env = {**os.environ, "GIT_TERMINAL_PROMPT": "0", "GCM_INTERACTIVE": "never"}
    try:
        p = _git(root, "push", remoto, f"HEAD:refs/heads/{rama}", env=env, timeout=PUSH_TIMEOUT_S)
        rep["push"] = "ok" if p.returncode == 0 else f"rechazado: {(p.stderr or p.stdout).strip()[-300:]}"
    except subprocess.TimeoutExpired:
        rep["push"] = f"timeout a los {PUSH_TIMEOUT_S} s: el commit queda local"
    return rep


def _restaurar(root: pathlib.Path, orig: dict[str, bytes | None]) -> None:
    for r, data in orig.items():
        _atomico(root / r, data)


def upgrade(repo: str) -> dict:
    """Aplica la lista cerrada de arreglos seguros (tickets 03, 04, 07) y commitea con push."""
    root = pathlib.Path(repo).resolve()
    if not (root / "sdlc.toml").exists():
        raise Rechazo(4, f"{root} no tiene sdlc.toml: adoptar es una decision del usuario (python -m sdlc init)")
    c = _ctx(root)
    rama = _precheck(root, c.cfg, [])
    antes = _diagnostico(root)
    fixes = [h for h in antes if h.fix]
    if not fixes:
        return {"repo": str(root), "hecho": [], "commit": None}
    rutas = sorted({h.ruta for h in fixes})
    _precheck(root, c.cfg, rutas)
    orig = {r: (root / r).read_bytes() if (root / r).exists() else None for r in rutas}
    for r in rutas:
        texto, eol = _leer(root / r)
        nuevo = texto
        for f in (h.fix for h in fixes if h.ruta == r and h.fix):
            nuevo = f(nuevo)
        if nuevo != texto:
            _atomico(root / r, _bytes(nuevo, eol))
    if "sdlc.toml" in rutas:
        try:
            nuevo_cfg = tomllib.loads(_leer(root / "sdlc.toml")[0])
            if c.cfg is not None and any(nuevo_cfg.get(k) != v for k, v in c.cfg.items()):
                raise ValueError("una clave previa cambio")
        except (tomllib.TOMLDecodeError, ValueError) as e:
            _restaurar(root, orig)
            raise Rechazo(1, f"sdlc.toml quedaba mal ({e}); restaurado") from e
    despues = _diagnostico(root)
    previas = {h.clave() for h in antes}
    nuevas = [h for h in despues if h.nivel == "desvio" and h.clave() not in previas]
    sin_cerrar = [h for h in fixes if h.completo and h.clave() in {x.clave() for x in despues}]
    if nuevas or sin_cerrar:
        _restaurar(root, orig)
        raise Rechazo(1, "el arreglo no convergio; restaurado: " + "; ".join(h.detalle.splitlines()[0]
                                                                             for h in nuevas + sin_cerrar))
    cambiadas = [r for r in rutas if ((root / r).read_bytes() if (root / r).exists() else None) != orig[r]]
    if not cambiadas:
        return {"repo": str(root), "hecho": [], "commit": None}
    hechos = [h for h in fixes if h.ruta in cambiadas]
    mensaje = ("sdlc convenciones: " + ", ".join(sorted({h.costumbre for h in hechos})) + "\n\n"
               + "".join(f"- {h.costumbre}: {h.ruta} ({h.detalle.splitlines()[0]})\n" for h in hechos))
    return {"repo": str(root), "hecho": [h.json() for h in hechos], **_commit_push(root, rama, cambiadas, mensaje)}


# --------------------------------------------------------------------------------------------------- areas

PROPUESTA = "docs/sdlc/areas-propuesta.toml"


def _llm_areas(prompt: str) -> str:
    from mmorch.config import DEFAULT_GENERATOR
    from mmorch.providers import call
    return call(DEFAULT_GENERATOR, prompt, node="sdlc-areas", phase="sdlc-areas").text


def _norma_area(d: str) -> str:
    return re.sub(r"[^a-z0-9_-]+", "-", d.lower()).strip("-") or "x"


def _propuesta_carpetas(c: Ctx) -> tuple[dict[str, list[str]], dict[str, list[str]]]:
    """Paso 1 sin LLM: un area por carpeta con codigo trackeado; los archivos sueltos de la raiz van a `raiz` por
    nombre exacto (en `_calza()` el `*` cruza carpetas, ticket 08). Devuelve (areas, archivos de cada area)."""
    exts = {e.lower() for e in ((c.cfg or {}).get("ext") or ["py"])}
    carpetas: dict[str, list[str]] = {}
    for f in c.archivos:
        partes = pathlib.PurePath(f).parts
        if pathlib.PurePath(f).suffix.lstrip(".").lower() not in exts:
            continue
        if any(p in P.SKIP_INIT or p.startswith(".") for p in partes):
            continue
        carpetas.setdefault(partes[0] if len(partes) > 1 else "", []).append(f)
    areas: dict[str, list[str]] = {}
    for d, fs in sorted(carpetas.items()):
        nombre = "raiz" if d == "" else _norma_area(d)
        if nombre in areas:
            raise Rechazo(1, f"dos carpetas dan el mismo nombre de area: {nombre}; renombra una o arma la propuesta a mano")
        areas[nombre] = sorted(fs) if d == "" else [f"{d}/**"]
    return areas, {n: sorted(_de_area(c, g)) for n, g in areas.items()}


def _titulo(p: pathlib.Path) -> str:
    for linea in _leer(p)[0].splitlines():
        if linea.startswith("# "):
            return linea[2:].strip()
    return p.stem


def _ajustes(c: Ctx, areas: dict[str, list[str]], prompt: str, llm: Callable[[str], str]) -> tuple[dict, list[str], int]:
    """Paso 2: el LLM sugiere unir o partir; un validador determinista aplica solo las operaciones validas."""
    try:
        texto = llm(prompt)
        m = re.search(r"\{.*\}", texto, re.S)
        ops = json.loads(m.group(0)).get("operaciones", []) if m else []
        if not isinstance(ops, list):
            ops = []
    except Exception:  # noqa: BLE001 - la propuesta por carpeta funciona sola si el LLM falla
        return areas, ["el LLM fallo: queda la propuesta por carpeta"], 0
    notas, descartadas = [], 0
    for op in ops:
        nuevo = _aplicar_op(c, areas, op) if isinstance(op, dict) else None
        if nuevo is None:
            descartadas += 1
            continue
        areas = nuevo
        motivo = str(op.get("motivo", "")).replace("\n", " ")
        if op.get("op") == "unir":
            notas.append(f"Ajuste del LLM: unir {', '.join(op['areas'])} -> {op['nombre']}: {motivo}")
        else:
            notas.append(f"Ajuste del LLM: partir {op['area']} -> {', '.join(op['en'])}: {motivo}")
    return areas, notas, descartadas


def _aplicar_op(c: Ctx, areas: dict[str, list[str]], op: dict) -> dict | None:
    if op.get("op") == "unir":
        juntas, nombre = op.get("areas"), op.get("nombre")
        if not (isinstance(juntas, list) and len(juntas) >= 2 and all(a in areas for a in juntas)
                and isinstance(nombre, str) and NOMBRE.match(nombre) and (nombre not in areas or nombre in juntas)):
            return None
        out = {k: v for k, v in areas.items() if k not in juntas}
        out[nombre] = sorted({g for a in juntas for g in areas[a]})
        return out
    if op.get("op") == "partir":
        area, en = op.get("area"), op.get("en")
        if not (area in areas and isinstance(en, dict) and len(en) >= 2):
            return None
        if any(not (isinstance(n, str) and NOMBRE.match(n)) or (n in areas and n != area) for n in en):
            return None
        if any(not (isinstance(g, list) and g and all(isinstance(x, str) and x for x in g)) for g in en.values()):
            return None
        partes = [_de_area(c, g) for g in en.values()]
        if any(not p for p in partes) or sum(map(len, partes)) != len(set().union(*partes)):
            return None
        if set().union(*partes) != _de_area(c, areas[area]):
            return None
        out = {k: v for k, v in areas.items() if k != area}
        out.update({n: list(g) for n, g in en.items()})
        return out
    return None


def _toml_tabla(nombre: str, tabla: dict) -> str:
    def valor(v):
        return "[" + ", ".join(json.dumps(x, ensure_ascii=False) for x in v) + "]" if isinstance(v, list) \
            else json.dumps(v, ensure_ascii=False)
    return f"[{nombre}]\n" + "".join(f"{k} = {valor(v)}\n" for k, v in sorted(tabla.items()))


def areas(repo: str, aprobar: bool = False, llm: Callable[[str], str] | None = None) -> dict:
    root = pathlib.Path(repo).resolve()
    if not (root / "sdlc.toml").exists():
        raise Rechazo(4, f"{root} no tiene sdlc.toml")
    c = _ctx(root)
    if c.cfg is None:
        raise Rechazo(1, f"sdlc.toml roto: {c.error}")
    if "areas" in c.cfg:
        raise Rechazo(1, "el repo ya declara [areas]; cambiar areas declaradas es una edicion a mano de sdlc.toml")
    return _aprobar_areas(root, c) if aprobar else _proponer_areas(root, c, llm or _llm_areas)


def _proponer_areas(root: pathlib.Path, c: Ctx, llm: Callable[[str], str]) -> dict:
    if (root / PROPUESTA).exists():
        raise Rechazo(1, f"ya existe {PROPUESTA}; aprobala con --aprobar o borrala para proponer de nuevo")
    propuesta, archivos = _propuesta_carpetas(c)
    planas = [(str(p.relative_to(root)), _titulo(p)) for p in _specs_planas(root)]
    prompt = ("Sos un arquitecto. Agrupas el codigo de un repo en areas para escribir una spec viva por area.\n"
              "Propuesta por carpeta (area: archivos):\n"
              + "".join(f"- {n}: {', '.join(fs[:30])}{' ...' if len(fs) > 30 else ''}\n" for n, fs in archivos.items())
              + ("Specs planas del repo (titulo de cada una):\n" + "".join(f"- {r}: {t}\n" for r, t in planas)
                 if planas else "")
              + 'Responde SOLO JSON: {"operaciones": [{"op": "unir", "areas": ["a", "b"], "nombre": "x", "motivo": "..."}, '
              '{"op": "partir", "area": "a", "en": {"x": ["a/sub/**"], "y": ["a/otro/**"]}, "motivo": "..."}]}. '
              "Sin cambios: {\"operaciones\": []}. Nombres en [a-z0-9_-]+.")
    propuesta, notas, descartadas = _ajustes(c, propuesta, prompt, llm)
    cabeza = [f"Propuesta de areas para {root.name} (python -m sdlc areas). Editala y aprobala con:",
              f"  python -m sdlc areas {root} --aprobar", *notas]
    if descartadas:
        cabeza.append(f"Operaciones del LLM descartadas por el validador: {descartadas}")
    if planas:
        cabeza += ["Specs planas sin area: asignalas en [spec_de_area] (nombre = \"ruta\")."]
        cabeza += [f"  {r} — {t}" for r, t in planas]
    texto = "".join(f"# {x}\n" for x in cabeza) + "\n" + _toml_tabla("areas", propuesta) + "\n[spec_de_area]\n" \
        + "".join(f"# {_norma_area(pathlib.Path(r).stem)} = {json.dumps(r)}\n" for r, _ in planas)
    _atomico(root / PROPUESTA, texto.encode("utf-8"))
    return {"repo": str(root), "propuesta": PROPUESTA, "areas": propuesta, "notas": notas, "descartadas": descartadas}


def _aprobar_areas(root: pathlib.Path, c: Ctx) -> dict:
    p = root / PROPUESTA
    if not p.exists():
        raise Rechazo(1, f"no hay {PROPUESTA}: corre primero python -m sdlc areas <repo>")
    try:
        prop = tomllib.loads(_leer(p)[0])
    except tomllib.TOMLDecodeError as e:
        raise Rechazo(1, f"{PROPUESTA} rota: {e}") from e
    nuevas = {k: prop[k] for k in ("areas", "spec_de_area") if prop.get(k)}
    if not nuevas.get("areas"):
        raise Rechazo(1, f"{PROPUESTA} no declara ninguna area")
    malos = [h for h in _diagnostico(root, nuevas)
             if h.nivel == "desvio" and h.costumbre in ("areas", "spec-de-area", "rutas-absolutas")]
    if malos:
        raise Rechazo(1, "propuesta rechazada: " + "; ".join(f"{h.ruta}: {h.detalle}" for h in malos))
    rama = _precheck(root, c.cfg, ["sdlc.toml"])
    texto, eol = _leer(root / "sdlc.toml")
    orig = (root / "sdlc.toml").read_bytes()
    nuevo = texto
    for k in ("areas", "spec_de_area"):
        if k in nuevas:
            nuevo = _fin(nuevo) + "\n" + _toml_tabla(k, nuevas[k])
    _atomico(root / "sdlc.toml", _bytes(nuevo, eol))
    leido = tomllib.loads(_leer(root / "sdlc.toml")[0])
    if any(leido.get(k) != v for k, v in {**(c.cfg or {}), **nuevas}.items()):
        _atomico(root / "sdlc.toml", orig)
        raise Rechazo(1, "sdlc.toml quedaba distinto de la propuesta; restaurado")
    rep = _commit_push(root, rama, ["sdlc.toml"], "sdlc convenciones: areas\n\n"
                       + "".join(f"- {n}: {', '.join(g)}\n" for n, g in sorted(nuevas["areas"].items())))
    if rep.get("commit"):
        p.unlink()
    return {"repo": str(root), "areas": nuevas["areas"], **rep}


# --------------------------------------------------------------------------------------------------- propiedades

def _nombre_req(s: str) -> str:
    m = re.match(r"\s*([A-Za-z0-9_.\-]+)", s)
    return m.group(1).lower().replace("_", "-") if m else ""


def _pyproject_reqs(cfg: dict) -> list[str]:
    proj = cfg.get("project", {})
    out = list(proj.get("dependencies", []))
    for grupo in [*proj.get("optional-dependencies", {}).values(), *cfg.get("dependency-groups", {}).values()]:
        out += [x for x in grupo if isinstance(x, str)]
    return out


def _package_json(c: Ctx) -> pathlib.Path | None:
    if (c.root / "package.json").exists():
        return c.root / "package.json"
    cand = sorted((f for f in c.archivos if pathlib.PurePath(f).name == "package.json" and "node_modules" not in f),
                  key=lambda f: (f.count("/"), f))
    return c.root / cand[0] if cand else None


def _necesarias(c: Ctx) -> list[dict]:
    """Librerias de propiedades que el repo necesita (ticket 09): py -> hypothesis, ts -> fast-check; Java no."""
    exts = {e.lower() for e in ((c.cfg or {}).get("ext") or ["py"])}
    out = []
    if "py" in exts:
        reqs, deps = [], None
        if (c.root / "pyproject.toml").exists():
            deps = "pyproject.toml"
            try:
                reqs += _pyproject_reqs(tomllib.loads(_leer(c.root / "pyproject.toml")[0]))
            except tomllib.TOMLDecodeError:
                pass
        for r in sorted(c.root.glob("requirements*.txt")):
            deps = deps or ("requirements.txt" if (c.root / "requirements.txt").exists() else r.name)
            reqs += [x for x in _leer(r)[0].splitlines() if x.strip() and not x.lstrip().startswith(("#", "-"))]
        presente = "hypothesis" in {_nombre_req(x) for x in reqs} or any(
            any((c.root / v).glob(pat)) for v in (".venv", "venv")
            for pat in ("lib/python*/site-packages/hypothesis", "Lib/site-packages/hypothesis"))
        out.append({"lib": "hypothesis", "lang": "py", "deps": deps, "presente": presente})
    if exts & {"ts", "tsx"}:
        pj = _package_json(c)
        presente = False
        if pj is not None:
            try:
                data = json.loads(_leer(pj)[0] or "{}")
            except ValueError:
                data = {}
            presente = "fast-check" in {**data.get("dependencies", {}), **data.get("devDependencies", {})} \
                or (pj.parent / "node_modules" / "fast-check").is_dir()
        out.append({"lib": "fast-check", "lang": "ts", "deps": str(pj.relative_to(c.root)) if pj else None,
                    "presente": presente})
    return out


def _sumar_a_dev(texto: str) -> str:
    """Suma "hypothesis" al extra `dev` de pyproject.toml como texto; el resto queda byte a byte igual."""
    lineas = texto.splitlines(True)
    cab = next((i for i, x in enumerate(lineas) if x.strip() == "[project.optional-dependencies]"), None)
    if cab is None:
        return _fin(texto) + '\n[project.optional-dependencies]\ndev = ["hypothesis"]\n'
    fin = next((i for i in range(cab + 1, len(lineas)) if lineas[i].startswith("[")), len(lineas))
    dev = next((i for i in range(cab + 1, fin) if re.match(r"\s*dev\s*=\s*\[", lineas[i])), None)
    if dev is None:
        lineas.insert(cab + 1, 'dev = ["hypothesis"]\n')
        return "".join(lineas)
    linea = lineas[dev]
    if "]" in linea.split("#", 1)[0]:
        cuerpo = linea.split("#", 1)[0]
        i = cuerpo.rindex("]")
        dentro = cuerpo[cuerpo.index("[") + 1:i].strip()
        lineas[dev] = linea[:i] + (", " if dentro and not dentro.endswith(",") else "") + '"hypothesis"' + linea[i:]
        return "".join(lineas)
    cierre = next(i for i in range(dev + 1, len(lineas)) if lineas[i].strip().startswith("]"))
    previa = next((i for i in range(cierre - 1, dev, -1) if lineas[i].strip()), None)
    if previa is not None and not lineas[previa].split("#", 1)[0].rstrip().endswith(","):
        lineas[previa] = re.sub(r"(\S)(\s*(#.*)?)$", r"\1,\2", lineas[previa].rstrip("\n")) + "\n"
    muestra = lineas[previa] if previa is not None else "    x"
    sangria = muestra[:len(muestra) - len(muestra.lstrip())] or "    "
    lineas.insert(cierre, f'{sangria}"hypothesis",\n')
    return "".join(lineas)


def _pinea(texto: str) -> bool:
    return any("==" in x for x in texto.splitlines() if x.strip() and not x.lstrip().startswith(("#", "-")))


def _plan_propiedades(c: Ctx) -> list[dict]:
    pasos = []
    py = P._python_de(c.root, c.cfg or {})
    for d in _necesarias(c):
        if d["presente"]:
            continue
        if d["lang"] == "py":
            if (c.root / "uv.lock").exists():
                pasos.append({"lib": d["lib"], "cmd": ["uv", "add", "--optional", "dev", "hypothesis"], "cwd": ".",
                              "rutas": ["pyproject.toml", "uv.lock"]})
            elif (c.root / "pyproject.toml").exists():
                pasos.append({"lib": d["lib"], "editar": "pyproject.toml", "pip": py, "rutas": ["pyproject.toml"]})
            elif (c.root / "requirements.txt").exists():
                pasos.append({"lib": d["lib"], "editar": "requirements.txt", "pip": py, "rutas": ["requirements.txt"]})
            else:
                pasos.append({"lib": d["lib"], "aviso": "sin pyproject.toml ni requirements.txt: no hay donde declararla"})
        else:
            if d["deps"] is None:
                pasos.append({"lib": d["lib"], "aviso": "sin package.json: no hay donde declararla"})
                continue
            carpeta = str(pathlib.PurePath(d["deps"]).parent)
            lock = str(pathlib.PurePath(carpeta) / "package-lock.json") if carpeta != "." else "package-lock.json"
            pasos.append({"lib": d["lib"], "cmd": ["npm", "install", "-D", "fast-check"], "cwd": carpeta,
                          "rutas": [d["deps"], lock]})
            if (c.cfg or {}).get("contenedor"):
                df = c.root / "Dockerfile.sdlc"
                if not df.exists():
                    pasos.append({"lib": d["lib"], "aviso": "contenedor sin Dockerfile.sdlc: la imagen no se puede cubrir"})
                else:
                    hs = _chequeo_bloque(_leer(df)[0], "propiedades", "Dockerfile.sdlc", "dockerfile", requerido=True)
                    if any(h.fix is None for h in hs):
                        pasos.append({"lib": d["lib"], "aviso": "bloque de Dockerfile.sdlc editado o roto: no se toca"})
                    elif hs:
                        pasos.append({"lib": d["lib"], "docker": hs[0].fix, "imagen": (c.cfg or {})["contenedor"],
                                      "rutas": ["Dockerfile.sdlc"]})
    return pasos


def _correr(cmd: list[str], cwd: pathlib.Path, run: Callable) -> subprocess.CompletedProcess:
    return run(cmd, cwd=cwd, capture_output=True, text=True)


def _version_pip(py: str, root: pathlib.Path, run: Callable) -> str | None:
    r = _correr([py, "-m", "pip", "show", "hypothesis"], root, run)
    m = re.search(r"^Version:\s*(\S+)", r.stdout or "", re.M)
    return m.group(1) if r.returncode == 0 and m else None


def propiedades(repo: str, aprobar: bool = False, run: Callable = subprocess.run) -> dict:
    root = pathlib.Path(repo).resolve()
    if not (root / "sdlc.toml").exists():
        raise Rechazo(4, f"{root} no tiene sdlc.toml")
    c = _ctx(root)
    pasos = _plan_propiedades(c)
    plan = [{k: v for k, v in p.items() if k != "docker"} | ({"docker": "bloque propiedades en Dockerfile.sdlc y "
            f"build de la imagen {p['imagen']}"} if "docker" in p else {}) for p in pasos]
    if not aprobar or not any("rutas" in p for p in pasos):
        return {"repo": str(root), "plan": plan, "hecho": []}
    rutas = sorted({r for p in pasos for r in p.get("rutas", [])})
    rama = _precheck(root, c.cfg, rutas)
    orig = {r: (root / r).read_bytes() if (root / r).exists() else None for r in rutas}
    avisos = [p["aviso"] for p in pasos if "aviso" in p]
    try:
        for p in pasos:
            if "cmd" in p:
                r = _correr(p["cmd"], root / p["cwd"], run)
                if r.returncode:
                    raise Rechazo(1, f"{' '.join(p['cmd'])} fallo: {(r.stderr or r.stdout or '').strip()[-300:]}")
            elif "editar" in p:
                version = None
                if p["pip"]:
                    r = _correr([p["pip"], "-m", "pip", "install", "hypothesis"], root, run)
                    if r.returncode:
                        raise Rechazo(1, f"pip install hypothesis fallo: {(r.stderr or r.stdout or '').strip()[-300:]}")
                    version = _version_pip(p["pip"], root, run)
                else:
                    avisos.append("sin .venv: hypothesis queda declarada y sin instalar")
                f = root / p["editar"]
                texto, eol = _leer(f)
                if p["editar"] == "pyproject.toml":
                    nuevo = _sumar_a_dev(texto)
                    leido = tomllib.loads(nuevo)
                    if "hypothesis" not in leido.get("project", {}).get("optional-dependencies", {}).get("dev", []):
                        raise Rechazo(1, "pyproject.toml no quedo con hypothesis en el extra dev")
                else:
                    linea = f"hypothesis=={version}" if version and _pinea(texto) else "hypothesis"
                    nuevo = _fin(texto) + linea + "\n"
                _atomico(f, _bytes(nuevo, eol))
            elif "docker" in p:
                f = root / "Dockerfile.sdlc"
                texto, eol = _leer(f)
                _atomico(f, _bytes(p["docker"](texto), eol))
                from mmorch.sandbox import engine
                b = _correr([engine() or "docker", "build", "-q", "-f", "Dockerfile.sdlc", "-t", p["imagen"], "."], root, run)
                if b.returncode:
                    raise Rechazo(1, f"la imagen {p['imagen']} no se construyo: {(b.stderr or '').strip()[-300:]}")
    except Rechazo:
        _restaurar(root, orig)
        raise
    cambiadas = [r for r in rutas if ((root / r).read_bytes() if (root / r).exists() else None) != orig[r]]
    if not cambiadas:
        return {"repo": str(root), "plan": plan, "hecho": [], "avisos": avisos}
    libs = sorted({p["lib"] for p in pasos if "rutas" in p})
    rep = _commit_push(root, rama, cambiadas, f"sdlc convenciones: propiedades ({', '.join(libs)})\n\n"
                       + "".join(f"- {r}\n" for r in cambiadas))
    return {"repo": str(root), "plan": plan, "hecho": cambiadas, "avisos": avisos, **rep}


# --------------------------------------------------------------------------------------------------- CLI

def _imprimir(rep) -> None:
    print(json.dumps(rep, ensure_ascii=False, indent=1))


def main(argv: list[str]) -> int:
    uso = ("uso: python -m sdlc doctor <repo>|--todos [raiz] | init --upgrade <repo> | areas <repo> [--aprobar] | "
           "propiedades <repo> [--aprobar]")
    try:
        if argv[:1] == ["doctor"]:
            if argv[1:2] == ["--todos"]:
                reps = [doctor(r) for r in repos(argv[2] if len(argv) > 2 else None)]
                _imprimir(reps)
                codigos = {codigo(r) for r in reps}
                return 1 if 1 in codigos else 3 if 3 in codigos else 0
            if len(argv) > 2:
                print(uso, file=sys.stderr)
                return 2
            rep = doctor(argv[1] if len(argv) > 1 else ".")
            _imprimir(rep)
            return codigo(rep)
        if argv[:2] == ["init", "--upgrade"]:
            resto = argv[2:]
            if "--todos" in resto:
                print("--todos solo existe en doctor: --upgrade actua sobre un repo por invocacion", file=sys.stderr)
                return 2
            if len(resto) > 1:
                print(uso, file=sys.stderr)
                return 2
            rep = upgrade(resto[0] if resto else ".")
            _imprimir(rep)
            return 1 if rep.get("error") else 0
        if argv[:1] in (["areas"], ["propiedades"]):
            resto = [a for a in argv[1:] if a != "--aprobar"]
            if len(resto) > 1:
                print(uso, file=sys.stderr)
                return 2
            f = areas if argv[0] == "areas" else propiedades
            rep = f(resto[0] if resto else ".", aprobar="--aprobar" in argv)
            _imprimir(rep)
            return 1 if rep.get("error") else 0
    except Rechazo as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return e.codigo
    print(uso, file=sys.stderr)
    return 2
