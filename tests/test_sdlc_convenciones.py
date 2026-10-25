"""repos-convenciones: `sdlc doctor`, `init --upgrade`, `areas` y `propiedades` (mapa `.scratch/repos-convenciones/`).

Un test por ticket con los puntos del test de aceptacion aprobado (`## Answer` de cada ticket). Repos git en tmp_path con
HOME temporal; el remoto es un repo bare local; el LLM, el gestor de paquetes y `docker build` van simulados.
"""
import json
import os
import pathlib
import socket
import subprocess

import pytest

import sdlc.convenciones as C
import sdlc.pipeline as P
from sdlc.agentes import _hash

LLM_REAL = C._llm_areas
SEPT_TOML = ('accept_cmd = "python -m pytest -q"\nsuite = ["tests", "-q"]\next = ["py"]\napprove_accept = true\n'
             'usd_max = 3.0\nfiles = [\n    "pkg/*.py",\n]\n')


@pytest.fixture(autouse=True)
def env(tmp_path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    for k in ("GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME"):
        monkeypatch.setenv(k, "t")
    for k in ("GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL"):
        monkeypatch.setenv(k, "t@t")
    return tmp_path


def _git(root, *args):
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout


def _repo(path: pathlib.Path, files: dict, *, remoto: bool = False, rama: str = "main") -> pathlib.Path:
    path.mkdir(parents=True, exist_ok=True)
    _git(path, "init", "-q", "-b", rama)
    for rel, data in files.items():
        p = path / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data if isinstance(data, bytes) else data.encode("utf-8"))
    _git(path, "add", "-A")
    _git(path, "commit", "-q", "--allow-empty", "-m", "base")
    if remoto:
        bare = path.parent / f"{path.name}-remoto.git"
        _git(path.parent, "init", "-q", "--bare", str(bare))
        _git(path, "remote", "add", "origin", str(bare))
        _git(path, "push", "-q", "-u", "origin", rama)
    return path


def _tpl() -> dict:
    return {f"docs/sdlc/{t.name}": t.read_text(encoding="utf-8") for t in P.TPL.glob("*.md")}


def _conforme(path: pathlib.Path, **kw) -> pathlib.Path:
    files = {"sdlc.toml": SEPT_TOML, "pkg/a.py": "x = 1\n", "tests/test_a.py": "def test_a():\n    pass\n",
             "AGENTS.md": "# r\n\n" + C.puntero_marcado(), "CLAUDE.md": "@AGENTS.md\n\n# r\n",
             ".gitignore": ".env\ncredentials.json\ntoken.json\n*.key\n", ".beads/config.yaml": "x: 1\n",
             "GOAL.md": "# metas\n", **_tpl()}
    files.update(kw.pop("files", {}))
    return _repo(path, files, **kw)


def _desvios(rep) -> set:
    return {h["costumbre"] for h in rep["desvios"]}


def _avisos(rep) -> set:
    return {h["costumbre"] for h in rep["avisos"]}


def _head(root) -> str:
    return _git(root, "rev-parse", "HEAD").strip()


@pytest.fixture
def sin_llm(monkeypatch):
    def prohibido(*a, **k):
        raise AssertionError("llamada a un LLM")
    import mmorch.providers
    monkeypatch.setattr(mmorch.providers, "call", prohibido)
    monkeypatch.setattr(P, "call", prohibido)
    monkeypatch.setattr(C, "_llm_areas", prohibido)


@pytest.fixture
def sin_red(monkeypatch):
    def prohibido(*a, **k):
        raise AssertionError("acceso a la red")
    monkeypatch.setattr(socket.socket, "connect", prohibido)


# ---------------------------------------------------------------------------------------------- ticket 01

def test_lista_unica_de_costumbres(env, sin_red, sin_llm):
    # 1. cada fila tiene un chequeo determinista; corre sin red (sin_red) y da una lista
    assert C.COSTUMBRES and all(callable(f) and isinstance(i, str) and g for i, g, f in C.COSTUMBRES)
    vacio = _repo(env / "vacio", {"sdlc.toml": "", "tests/test_a.py": "def test_a():\n    pass\n",
                                  "docs/sdlc/run-log.json": '{"x": 1}\n'})
    rep = C.doctor(str(vacio))
    # 2. un repo vacio muestra un desvio por cada costumbre de SDLC, secretos y .beads/
    assert {"toml", "plantillas", "agents", "gitignore", "beads"} <= _desvios(rep)
    con_area = _repo(env / "area", {"sdlc.toml": 'files = ["*.py"]\n[areas]\nx = ["nada/**"]\n', "a.py": ""})
    assert "areas" in _desvios(C.doctor(str(con_area)))  # el grupo SDD chequea solo repos con areas
    assert "areas" not in _desvios(rep)
    # 3. tras --upgrade no quedan desvios de SDLC, SDD ni .gitignore; 4. .beads/ sigue marcado
    C.upgrade(str(vacio))
    rep2 = C.doctor(str(vacio))
    assert _desvios(rep2) == {"beads"}
    # 5. nunca escribe docs/claude, tests/README.md, constitucion, areas ni artefactos de corrida
    assert not (vacio / "docs" / "claude").exists() and not (vacio / "tests" / "README.md").exists()
    assert not (vacio / "GOAL.md").exists() and "areas" not in (vacio / "sdlc.toml").read_text(encoding="utf-8")
    assert (vacio / "docs/sdlc/run-log.json").read_text(encoding="utf-8") == '{"x": 1}\n'
    # 6. constitucion ausente y YAML fuera de la lista son avisos, y ningun aviso escribe
    solo_avisos = _conforme(env / "avisos", files={"datos.yaml": "a: 1\n"})
    (solo_avisos / "GOAL.md").unlink()
    _git(solo_avisos, "commit", "-qam", "sin goal")
    r = C.doctor(str(solo_avisos))
    assert {"constitucion", "formato"} <= _avisos(r) and not r["desvios"]
    head = _head(solo_avisos)
    assert C.upgrade(str(solo_avisos))["commit"] is None and _head(solo_avisos) == head
    # 7. un secreto trackeado se reporta y no sale del indice
    sec = _conforme(env / "sec")
    (sec / "token.json").write_text("{}")
    _git(sec, "add", "-f", "token.json")
    _git(sec, "commit", "-qm", "secreto")
    assert any(h["ruta"] == "token.json" for h in C.doctor(str(sec))["desvios"])
    C.upgrade(str(sec))
    assert "token.json" in _git(sec, "ls-files").split()
    # 8. un repo conforme muestra cero desvios
    ok = _conforme(env / "ok")
    assert C.doctor(str(ok))["desvios"] == []
    # 9. reporte JSON con listas separadas; 10. exit code de tres estados; 11. byte a byte estable
    assert set(json.loads(json.dumps(C.doctor(str(ok))))) >= {"desvios", "avisos"}
    limpio = _conforme(env / "limpio", files={"pyproject.toml": '[project]\nname = "x"\ndependencies = ["hypothesis"]\n'})
    assert C.main(["doctor", str(limpio)]) == 0
    assert C.main(["doctor", str(solo_avisos)]) == 3
    assert C.main(["doctor", str(_repo(env / "mal", {"sdlc.toml": ""}))]) == 1
    assert json.dumps(C.doctor(str(vacio))) == json.dumps(C.doctor(str(vacio)))


# ---------------------------------------------------------------------------------------------- ticket 02

def test_lista_de_repos(env, sin_red):
    raiz = env / "raiz"
    a = _repo(raiz / "a", {"sdlc.toml": SEPT_TOML})
    anidado = _repo(a / "anidado", {"sdlc.toml": SEPT_TOML})
    _repo(raiz / "sin", {"x.txt": "x"})
    _repo(raiz / ".oculto" / "r", {"sdlc.toml": SEPT_TOML})
    _repo(raiz / "node_modules" / "r", {"sdlc.toml": SEPT_TOML})
    roto = _repo(raiz / "roto", {"sdlc.toml": "esto no es toml ["})
    wt = raiz / "wt"
    _git(a, "worktree", "add", "-q", str(wt))  # .git archivo + sdlc.toml
    (raiz / "link").symlink_to(a)
    (raiz / "ciclo").symlink_to(raiz)
    cerrada = raiz / "cerrada"
    _repo(cerrada / "r", {"sdlc.toml": SEPT_TOML})
    cerrada.chmod(0)
    try:
        lista = C.repos(str(raiz))
        # 1. solo repos con sdlc.toml, ordenados; 2. el anidado entra; 3. el worktree no; 4. carpeta con punto no;
        # 8. symlinks no se siguen; 9. node_modules y carpetas sin permiso se saltan sin error
        assert lista == sorted([str(a), str(anidado), str(roto)])
        # 7. dos corridas dan la misma lista
        assert C.repos(str(raiz)) == lista
    finally:
        cerrada.chmod(0o755)
    # 5. sin sdlc.toml: no entra y doctor sale con su codigo propio
    assert C.main(["doctor", str(raiz / "sin")]) == 4
    # 10. un sdlc.toml roto entra y es desvio
    assert "toml" in _desvios(C.doctor(str(roto)))
    # 6. rama detras de su ref remota: aviso, sin red (el fetch del test es local y anterior al doctor)
    r1 = _conforme(env / "r1", remoto=True)
    otro = env / "otro"
    _git(env, "clone", "-q", "-b", "main", str(env / "r1-remoto.git"), str(otro))
    (otro / "n.txt").write_text("n")
    _git(otro, "add", "-A")
    _git(otro, "commit", "-qm", "n")
    _git(otro, "push", "-q")
    _git(r1, "fetch", "-q")
    assert "rama" in _avisos(C.doctor(str(r1)))


# ---------------------------------------------------------------------------------------------- ticket 03

def test_actualizar_converge(env, sin_llm, monkeypatch):
    legacy = "# r\n\n## SDLC (pipeline de 6 etapas)\n\nEste repo construye features.\n"
    files = {"sdlc.toml": SEPT_TOML, "pkg/a.py": "x = 1\n", "tests/test_a.py": "", "AGENTS.md": legacy,
             ".gitignore": "*.pyc", "GOAL.md": "#\n", **_tpl()}
    del files["docs/sdlc/refutar-tests.md"]
    repo = _repo(env / "sept", files, remoto=True)
    antes = {r: (repo / r).read_text(encoding="utf-8") for r in files}
    # 1. doctor nombra cada desvio
    rep = C.doctor(str(repo))
    assert {"plantillas", "gitignore", "beads"} <= _desvios(rep)
    assert any(h["ruta"] == "docs/sdlc/refutar-tests.md" for h in rep["desvios"])
    # 2. tras --upgrade solo quedan .beads/ y claves sin valor adivinable; 4. sin LLM (sin_llm)
    out = C.upgrade(str(repo))
    assert out["commit"] and _desvios(C.doctor(str(repo))) == {"beads"}
    # 5. cada linea previa sigue presente y en orden
    for r, viejo in antes.items():
        nuevas = (repo / r).read_text(encoding="utf-8").splitlines()
        it = iter(nuevas)
        assert all(linea in it for linea in viejo.splitlines()), r
    # 3. una segunda corrida deja el arbol limpio y no commitea
    head = _head(repo)
    assert C.upgrade(str(repo))["commit"] is None and _head(repo) == head
    assert _git(repo, "status", "--porcelain") == ""
    # 15. ningun archivo temporal
    assert not list(repo.rglob(".*.sdlc-tmp-*"))
    # 9. no escribe claves con default en codigo
    assert set(C.tomllib.loads((repo / "sdlc.toml").read_text(encoding="utf-8"))) == set(C.tomllib.loads(SEPT_TOML))
    # 6. sdlc.toml que termina con [sprites]: la clave queda en la raiz
    spr = _conforme(env / "spr", files={"sdlc.toml": 'accept_cmd = "x"\nusd_max = 3.0\n[sprites]\ndir = "s"\n'})
    C.upgrade(str(spr))
    leido = C.tomllib.loads((spr / "sdlc.toml").read_text(encoding="utf-8"))
    assert "files" in leido and "files" not in leido["sprites"]
    # 7. sin sdlc.toml: no escribe nada y sale con codigo propio
    nada = _repo(env / "nada", {"a.py": ""})
    with pytest.raises(C.Rechazo) as e:
        C.upgrade(str(nada))
    assert e.value.codigo == 4 and _git(nada, "status", "--porcelain") == ""
    # 8. clave desconocida: aviso e intacta
    raro = _conforme(env / "raro", files={"sdlc.toml": SEPT_TOML.replace("usd_max", "rara = 1\nusd_max")})
    assert "toml-clave" in _avisos(C.doctor(str(raro)))
    C.upgrade(str(raro))
    assert "rara = 1" in (raro / "sdlc.toml").read_text(encoding="utf-8")
    # 10. docs/sdlc/sdlc.toml: desvio, _cfg lo ignora y --upgrade no lo borra
    leg = _conforme(env / "leg", files={"docs/sdlc/sdlc.toml": "usd_max = 99.0\n"})
    assert "toml-legacy" in _desvios(C.doctor(str(leg)))
    monkeypatch.setattr(P, "WT", leg)
    monkeypatch.setattr(P, "FEAT", None)
    assert P._cfg()["usd_max"] == 3.0
    C.upgrade(str(leg))
    assert (leg / "docs/sdlc/sdlc.toml").exists()
    # 11. si tomllib falla al releer, sdlc.toml vuelve a sus bytes
    rot = _conforme(env / "rot", files={"sdlc.toml": 'accept_cmd = "x"\n'})
    orig = (rot / "sdlc.toml").read_bytes()
    monkeypatch.setattr(C, "_antes_de_tablas", lambda t, linea: t + "esto no es [toml\n")
    with pytest.raises(C.Rechazo):
        C.upgrade(str(rot))
    assert (rot / "sdlc.toml").read_bytes() == orig
    monkeypatch.undo()
    # 13. repo conforme: --upgrade no escribe
    ok = _conforme(env / "ok")
    head = _head(ok)
    assert C.upgrade(str(ok))["commit"] is None and _head(ok) == head and _git(ok, "status", "--porcelain") == ""


def test_build_feature_registra_doctor(env, monkeypatch):
    repo = _repo(env / "bf", {"sdlc.toml": 'accept_cmd = "true"\nfiles = ["*.py"]\n', "a.py": ""})
    monkeypatch.setattr(P, "run", lambda from_stage: dict(P.state))
    # 12. el reporte de doctor queda en el run-log y la corrida sigue con desvios
    st = P.build_feature("x", "t", str(repo), accept_cmd="true", wt=env / "wt")
    assert st["doctor"]["desvios"]
    # 14. un doctor que lanza deja el error en el run-log y la corrida sigue
    monkeypatch.setattr(C, "doctor", lambda r: (_ for _ in ()).throw(RuntimeError("boom")))
    st = P.build_feature("x", "t", str(repo), accept_cmd="true", wt=env / "wt2")
    assert "boom" in st["doctor"]["error"]


# ---------------------------------------------------------------------------------------------- ticket 04

VIEJA = "# plantilla vieja\n"


def test_respeta_ediciones(env, monkeypatch):
    monkeypatch.setattr(C, "PUBLICADAS", {k: [*v, _hash(VIEJA)] for k, v in C.PUBLICADAS.items()})
    v1 = C.BLOQUES["puntero"][1]
    monkeypatch.setitem(C.BLOQUES, "puntero", {1: v1, 2: v1 + "\nVersion 2.\n"})
    toml = "# comentario propio\n" + SEPT_TOML.replace('"pkg/*.py"', '"pkg/*.py",  # curado')
    agents = "# r\n\nParrafo propio del usuario.\n\n" + C.bloque("puntero", "AGENTS.md", 1)
    tpl = _tpl()
    tpl["docs/sdlc/spec-template.md"] = VIEJA
    tpl["docs/sdlc/spec-review.md"] += "linea propia\n"
    repo = _conforme(env / "ed", files={"sdlc.toml": toml, "AGENTS.md": agents, "CLAUDE.md": "# sin import\n", **tpl})
    rep = C.doctor(str(repo))
    editada = [h for h in rep["avisos"] if h["ruta"] == "docs/sdlc/spec-review.md"]
    # 4. plantilla editada: aviso con ruta y diff
    assert editada and "linea propia" in editada[0]["detalle"]
    C.upgrade(str(repo))
    # 1. y 10. sdlc.toml intacto y legible; 2. parrafo propio intacto
    assert (repo / "sdlc.toml").read_text(encoding="utf-8") == toml
    assert C.tomllib.loads(toml) == C.tomllib.loads((repo / "sdlc.toml").read_text(encoding="utf-8"))
    ag = (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert "Parrafo propio del usuario." in ag
    # 3. plantilla vieja e intacta -> vigente; 4. editada intacta
    assert (repo / "docs/sdlc/spec-template.md").read_text(encoding="utf-8") == tpl_vigente("spec-template.md")
    assert (repo / "docs/sdlc/spec-review.md").read_text(encoding="utf-8").endswith("linea propia\n")
    # 5. bloque v:1 con hash correcto -> interior y marca v:2
    assert "v:2 hash:" in ag and "Version 2." in ag
    # 8. CLAUDE.md recibe @AGENTS.md al inicio; el resto igual
    assert (repo / "CLAUDE.md").read_text(encoding="utf-8") == "@AGENTS.md\n\n# sin import\n"
    # 6. interior que no coincide con su hash: intacto con aviso y diff
    roto = C.bloque("propiedades", "Dockerfile.sdlc").replace("fast-check", "fast-check otra")
    b = _conforme(env / "b", files={"Dockerfile.sdlc": "FROM x\n" + roto})
    h = [x for x in C.doctor(str(b))["avisos"] if x["ruta"] == "Dockerfile.sdlc"]
    assert h and "editado" in h[0]["detalle"] and "@@" in h[0]["detalle"]
    C.upgrade(str(b))
    assert (b / "Dockerfile.sdlc").read_text(encoding="utf-8") == "FROM x\n" + roto
    # 7. puntero viejo sin cierre: intacto con aviso
    leg = _conforme(env / "leg", files={"AGENTS.md": "# r\n\n## SDLC (pipeline de 6 etapas)\n\nTexto.\n"})
    assert "agents" in _avisos(C.doctor(str(leg)))
    C.upgrade(str(leg))
    assert (leg / "AGENTS.md").read_text(encoding="utf-8") == "# r\n\n## SDLC (pipeline de 6 etapas)\n\nTexto.\n"
    # 9. solo plantillas o bloques editados -> exit de "solo avisos"
    sa = _conforme(env / "sa", files={"docs/sdlc/spec-review.md": "editada\n",
                                      "pyproject.toml": '[project]\nname = "x"\ndependencies = ["hypothesis"]\n'})
    assert C.main(["doctor", str(sa)]) == 3
    # 11. v: mayor al publicado y plantilla con hash ausente: intactos con aviso
    nuevo = C.bloque("puntero", "AGENTS.md", 2).replace("v:2", "v:9")
    mayor = _conforme(env / "mayor", files={"AGENTS.md": "# r\n\n" + nuevo})
    assert any("mas nuevo" in x["detalle"] for x in C.doctor(str(mayor))["avisos"])
    C.upgrade(str(mayor))
    assert (mayor / "AGENTS.md").read_text(encoding="utf-8") == "# r\n\n" + nuevo
    # 12. copia CRLF de una version publicada: cuenta como intacta y sigue en CRLF
    crlf = _conforme(env / "crlf", files={"docs/sdlc/spec-template.md": VIEJA.replace("\n", "\r\n").encode()})
    C.upgrade(str(crlf))
    data = (crlf / "docs/sdlc/spec-template.md").read_bytes()
    assert data == tpl_vigente("spec-template.md").replace("\n", "\r\n").encode()
    # 13. BEGIN sin END o nombre duplicado: intacto, aviso que nombra la linea
    abierto = "# r\n\n<!-- BEGIN SDLC puntero v:1 hash:x -->\nsin cierre\n"
    br = _conforme(env / "br", files={"AGENTS.md": abierto})
    av = [x for x in C.doctor(str(br))["avisos"] if x["ruta"] == "AGENTS.md"]
    assert av and "linea 3" in av[0]["detalle"]
    C.upgrade(str(br))
    assert (br / "AGENTS.md").read_text(encoding="utf-8") == abierto


def tpl_vigente(nombre: str) -> str:
    return (P.TPL / nombre).read_text(encoding="utf-8")


def test_lista_de_hashes():
    for t in P.TPL.glob("*.md"):
        assert _hash(t.read_text(encoding="utf-8")) in C.PUBLICADAS[t.name], f"falta el hash de {t.name}"


# ---------------------------------------------------------------------------------------------- ticket 05

def test_sin_rutas_de_maquina(env, monkeypatch):
    def rutas(toml: str) -> list[str]:
        repo = _conforme(env / f"r{abs(hash(toml))}", files={"sdlc.toml": toml})
        return [h["detalle"] for h in C.doctor(str(repo))["desvios"] if h["costumbre"] == "rutas-absolutas"]
    base = 'accept_cmd = "x"\nfiles = ["src/**/*.ts"]\n'
    # 1. --basetemp C:/ en suite: desvio que nombra la clave
    d = rutas(base + 'suite = ["tests", "--basetemp", "C:/Users/x/t"]\n')
    assert d and d[0].startswith("suite[2]")
    # 2. / y ~; 3. ruta dentro de un comando; 4. --x=C:/ junto; 10. unidad en minuscula; 11. ruta de red
    for valor in ['"/home/u/x"', '"~/x"', '"npx tsc -p /abs/x.json"', '"pytest --basetemp=C:/x"', '"c:/x"',
                  '"\\\\\\\\srv\\\\share"']:
        assert C._ruta_absoluta(C.tomllib.loads(f"v = {valor}")["v"]), valor
    # 8. comillas dentro del comando (shlex.split)
    assert C._ruta_absoluta("cmd --cfg '/abs dir/x'")
    # 5. globs relativos, imagenes y URLs no; 9. URL con ?a=/b no
    for ok in ["src/**/*.ts", "docker.io/library/node:22", "https://x.org/p?a=/b", "-p no:cacheprovider"]:
        assert C._ruta_absoluta(ok) is None, ok
    assert rutas(base + 'contenedor = "docker.io/library/node:22"\n') == []
    # 6. sin `suite`, build_feature usa el --basetemp de tempfile.gettempdir()
    repo = _repo(env / "bt", {"sdlc.toml": base, "a.py": ""})
    visto = {}
    monkeypatch.setattr(P, "configure", lambda t, **kw: visto.update(kw["feat"]))
    monkeypatch.setattr(P, "run", lambda from_stage: {})
    monkeypatch.setattr(C, "doctor", lambda r: {})
    P.build_feature("x", "t", str(repo), accept_cmd="true")
    assert visto["suite"][-1] == str(pathlib.Path(P.tempfile.gettempdir()) / "pyt-sdlc-wt")
    monkeypatch.undo()
    # 7. --upgrade no cambia una clave con ruta absoluta, y el reporte la nombra
    toml = SEPT_TOML.replace('["tests", "-q"]', '["tests", "--basetemp", "C:/Users/x/t"]')
    abs_repo = _conforme(env / "abs", files={"sdlc.toml": toml})
    C.upgrade(str(abs_repo))
    assert (abs_repo / "sdlc.toml").read_text(encoding="utf-8") == toml
    assert any("C:/Users/x/t" in h["detalle"] for h in C.doctor(str(abs_repo))["desvios"])


# ---------------------------------------------------------------------------------------------- ticket 06

def test_specs_previas(env):
    areas = '[areas]\ningesta = ["motor_fixture.py"]\nliquida = ["motor_liquidador.py"]\n'
    files = {"motor_fixture.py": "", "motor_liquidador.py": "", "docs/specs/01-ingesta.md": "# Ingesta\n",
             "docs/sdlc/features/x.md": "# Ficha\n"}

    def repo(nombre, extra_toml, mas=None):
        return _conforme(env / nombre, files={**files, **(mas or {}), "sdlc.toml": SEPT_TOML + extra_toml})
    # 1. un archivo suelto en docs/specs/ no da desvio ni aviso y --upgrade no lo toca
    suelto = repo("suelto", "")
    rep = C.doctor(str(suelto))
    assert not [h for h in rep["desvios"] + rep["avisos"] if h["ruta"].startswith("docs/specs/01")]
    C.upgrade(str(suelto))
    assert (suelto / "docs/specs/01-ingesta.md").read_text(encoding="utf-8") == "# Ingesta\n"
    # 7. una ficha ya mergeada queda en su lugar
    assert (suelto / "docs/sdlc/features/x.md").exists()
    # 2. con [spec_de_area], la spec base de ingesta es la plana; 3. sin entrada, la ruta default
    cfg = C.tomllib.loads(areas + '[spec_de_area]\ningesta = "docs/specs/01-ingesta.md"\n')
    assert C.spec_base(cfg, "ingesta") == "docs/specs/01-ingesta.md"
    assert C.spec_base(cfg, "liquida") == "docs/specs/liquida/spec.md"
    assert "spec-de-area" not in _desvios(C.doctor(str(repo("bien", areas + '[spec_de_area]\n'
                                                                       'ingesta = "docs/specs/01-ingesta.md"\n'))))

    def malo(nombre, entrada):
        return [h["detalle"] for h in C.doctor(str(repo(nombre, areas + "[spec_de_area]\n" + entrada)))["desvios"]
                if h["costumbre"] in ("spec-de-area", "rutas-absolutas")]
    # 4. area ausente; 5. archivo inexistente; 6. ruta absoluta; 8. sale del repo; 9. dos areas, misma spec
    assert any("no existe en [areas]" in d for d in malo("m4", 'otra = "docs/specs/01-ingesta.md"\n'))
    assert any("no existe docs/specs/nada.md" in d for d in malo("m5", 'ingesta = "docs/specs/nada.md"\n'))
    assert malo("m6", 'ingesta = "/abs/spec.md"\n')
    assert any("sale del repo" in d for d in malo("m8", 'ingesta = "../x.md"\n'))
    assert any("misma spec" in d for d in malo("m9", 'ingesta = "docs/specs/liquida/spec.md"\n'))


# ---------------------------------------------------------------------------------------------- ticket 07

def _con_arreglo(path, **kw):
    """Repo conforme al que le falta una plantilla: --upgrade tiene algo que hacer."""
    repo = _conforme(path, **kw)
    (repo / "docs/sdlc/refutar-tests.md").unlink()
    _git(repo, "commit", "-qam", "sin plantilla")
    if kw.get("remoto"):
        _git(repo, "push", "-q")
    return repo


def test_aplicar_en_un_repo(env, monkeypatch):
    # 1. un archivo sucio que hay que tocar aborta sin escrituras ni commit
    r1 = _conforme(env / "r1", files={".gitignore": "*.pyc\n"}, remoto=True)  # --upgrade tiene que tocar .gitignore
    (r1 / ".gitignore").write_text("*.pyc\nmio\n")
    head = _head(r1)
    with pytest.raises(C.Rechazo, match="sucios"):
        C.upgrade(str(r1))
    assert (r1 / ".gitignore").read_text() == "*.pyc\nmio\n" and _head(r1) == head
    # 2. sucio que no se toca sigue igual; 3. commit solo con sus rutas; 4. push al remoto; 14. mensaje fijo
    # 12. lo staged del usuario queda staged y fuera del commit
    r2 = _con_arreglo(env / "r2", remoto=True)
    (r2 / "pkg/a.py").write_text("x = 2\n")
    (r2 / "nuevo.txt").write_text("staged\n")
    _git(r2, "add", "nuevo.txt")
    out = C.upgrade(str(r2))
    assert (r2 / "pkg/a.py").read_text() == "x = 2\n"
    assert _git(r2, "show", "--name-only", "--format=", "HEAD").split() == ["docs/sdlc/refutar-tests.md"]
    assert out["push"] == "ok" and _git(env / "r2-remoto.git", "rev-parse", "main").strip() == _head(r2)
    assert _git(r2, "log", "-1", "--format=%s").strip() == "sdlc convenciones: plantillas"
    assert "A  nuevo.txt" in _git(r2, "status", "--porcelain")
    # 5. sin remoto: commit local y aviso
    r3 = _con_arreglo(env / "r3")
    out = C.upgrade(str(r3))
    assert out["commit"] and "sin remoto" in out["aviso"]
    # 6. push rechazado: el commit queda y el reporte lo dice; 15. sin --force ni reintento
    r4 = _con_arreglo(env / "r4", remoto=True)
    otro = env / "otro"
    _git(env, "clone", "-q", "-b", "main", str(env / "r4-remoto.git"), str(otro))
    (otro / "z.txt").write_text("z")
    _git(otro, "add", "-A")
    _git(otro, "commit", "-qm", "z")
    _git(otro, "push", "-q")
    pushes = []
    real = C._git

    def espia(root, *args, **kw):
        if args[:1] == ("push",):
            pushes.append(args)
        return real(root, *args, **kw)
    monkeypatch.setattr(C, "_git", espia)
    out = C.upgrade(str(r4))
    assert out["commit"] and out["push"].startswith("rechazado")
    assert len(pushes) == 1 and not any(a.startswith(("-f", "--force")) for a in pushes[0])
    monkeypatch.undo()
    # 16. push sin prompt y con timeout: un timeout conserva el commit con aviso
    r5 = _con_arreglo(env / "r5", remoto=True)
    vistos = {}

    def lento(root, *args, **kw):
        if args[:1] == ("push",):
            vistos.update(kw)
            raise subprocess.TimeoutExpired("git push", kw["timeout"])
        return real(root, *args, **kw)
    monkeypatch.setattr(C, "_git", lento)
    out = C.upgrade(str(r5))
    assert out["commit"] and "timeout" in out["push"] and vistos["env"]["GIT_TERMINAL_PROMPT"] == "0"
    monkeypatch.undo()
    # 7. un pre-commit que falla deja los cambios sin commitear y el reporte lo dice
    r6 = _con_arreglo(env / "r6")
    hook = r6 / ".git/hooks/pre-commit"
    hook.write_text("#!/bin/sh\nexit 1\n")
    hook.chmod(0o755)
    head = _head(r6)
    assert C.main(["init", "--upgrade", str(r6)]) == 1
    assert _head(r6) == head and (r6 / "docs/sdlc/refutar-tests.md").exists()
    # 8. rama que no es la default o HEAD separado: aborta y nombra la rama esperada
    r7 = _con_arreglo(env / "r7")
    _git(r7, "checkout", "-q", "-b", "feature")
    with pytest.raises(C.Rechazo, match="esperada main"):
        C.upgrade(str(r7))
    _git(r7, "checkout", "-q", "--detach", "main")
    with pytest.raises(C.Rechazo, match="HEAD separado"):
        C.upgrade(str(r7))
    assert not (r7 / "docs/sdlc/refutar-tests.md").exists()
    # 9. la clave `rama` gana sobre origin/HEAD
    r8 = _con_arreglo(env / "r8", files={"sdlc.toml": SEPT_TOML + 'rama = "trabajo"\n'}, remoto=True)
    _git(r8, "remote", "set-head", "origin", "main")
    assert C.rama_default(r8, C.tomllib.loads((r8 / "sdlc.toml").read_text())) == "trabajo"
    with pytest.raises(C.Rechazo, match="esperada trabajo"):
        C.upgrade(str(r8))
    # 10. sin origin/HEAD: la unica main local es la default; main y master juntas abortan
    r9 = _con_arreglo(env / "r9")
    assert C.rama_default(r9, {}) == "main"
    _git(r9, "branch", "master")
    assert C.rama_default(r9, {}) is None
    with pytest.raises(C.Rechazo, match="rama default"):
        C.upgrade(str(r9))
    # 11. --upgrade --todos falla con un mensaje
    assert C.main(["init", "--upgrade", "--todos"]) == 2
    # 13. sin arreglos pendientes: ni commit vacio ni push
    ok = _conforme(env / "ok", remoto=True)
    head = _head(ok)
    assert C.upgrade(str(ok))["commit"] is None and _head(ok) == head


# ---------------------------------------------------------------------------------------------- ticket 08

def _areas_repo(path, extra=None, **kw):
    files = {"a/x.py": "", "b/y.py": "", "c/z.py": "", "tests/test_t.py": "", "docs/d.py": "", ".h/w.py": ""}
    return _conforme(path, files={**files, **(extra or {})}, **kw)


def _llm(respuesta, prompts=None):
    def f(prompt):
        if prompts is not None:
            prompts.append(prompt)
        if isinstance(respuesta, Exception):
            raise respuesta
        return respuesta
    return f


def test_propuesta_de_areas(env, sin_llm, monkeypatch):
    # 1. tres carpetas -> tres areas en la propuesta; 16. salta tests/, docs/ y carpetas con punto
    r = _areas_repo(env / "r1", remoto=True)
    toml_antes, head = (r / "sdlc.toml").read_bytes(), _head(r)
    out = C.areas(str(r), llm=_llm('{"operaciones": []}'))
    assert out["areas"] == {"a": ["a/**"], "b": ["b/**"], "c": ["c/**"], "pkg": ["pkg/**"]}
    assert (r / C.PROPUESTA).exists()
    # 5. sin --aprobar: sdlc.toml intacto y sin commit
    assert (r / "sdlc.toml").read_bytes() == toml_antes and _head(r) == head
    # 15. una propuesta existente no se pisa
    with pytest.raises(C.Rechazo, match="ya existe"):
        C.areas(str(r), llm=_llm('{"operaciones": []}'))
    # 10. un area renombrada a mano entra con ese nombre
    prop = (r / C.PROPUESTA).read_text(encoding="utf-8").replace('pkg = ["pkg/**"]', 'nucleo = ["pkg/**"]')
    (r / C.PROPUESTA).write_text(prop, encoding="utf-8")
    # 6. --aprobar: [areas] al final y nombres validos; 7. sin propuesta, commit solo con sdlc.toml, push
    out = C.areas(str(r), aprobar=True)
    texto = (r / "sdlc.toml").read_text(encoding="utf-8")
    leido = C.tomllib.loads(texto)
    assert texto.startswith(toml_antes.decode()) and texto.rstrip().endswith('nucleo = ["pkg/**"]')
    assert set(leido["areas"]) == {"a", "b", "c", "nucleo"} and all(C.NOMBRE.match(n) for n in leido["areas"])
    assert not (r / C.PROPUESTA).exists()
    assert _git(r, "show", "--name-only", "--format=", "HEAD").split() == ["sdlc.toml"] and out["push"] == "ok"
    # 14. un repo que ya declara [areas] no recibe propuesta
    with pytest.raises(C.Rechazo, match="ya declara"):
        C.areas(str(r), llm=_llm("{}"))
    # 2. el LLM falla -> propuesta por carpeta; 4. operacion invalida o respuesta no JSON -> descartada
    for i, resp in enumerate([RuntimeError("caido"), "no es json",
                              '{"operaciones": [{"op": "unir", "areas": ["a", "zz"], "nombre": "q"}]}']):
        rr = _areas_repo(env / f"f{i}")
        assert C.areas(str(rr), llm=_llm(resp))["areas"]["a"] == ["a/**"]
    assert C.areas(str(_areas_repo(env / "f9")), llm=_llm(
        '{"operaciones": [{"op": "unir", "areas": ["a", "zz"], "nombre": "q"}]}'))["descartadas"] == 1
    # 3. unir valido: une y anota el motivo
    u = _areas_repo(env / "u")
    out = C.areas(str(u), llm=_llm('{"operaciones": [{"op": "unir", "areas": ["a", "b"], "nombre": "ab", '
                                   '"motivo": "comparten modelo"}]}'))
    assert out["areas"]["ab"] == ["a/**", "b/**"] and "comparten modelo" in (u / C.PROPUESTA).read_text()
    # 8. --aprobar rechaza un archivo en dos areas sin escribir
    s = _areas_repo(env / "s")
    C.areas(str(s), llm=_llm('{"operaciones": []}'))
    (s / C.PROPUESTA).write_text('[areas]\na = ["a/**"]\ntodo = ["**"]\n')
    antes = (s / "sdlc.toml").read_bytes()
    with pytest.raises(C.Rechazo, match="dos areas"):
        C.areas(str(s), aprobar=True)
    assert (s / "sdlc.toml").read_bytes() == antes
    # 9. --aprobar aborta fuera de la rama default o con sdlc.toml sucio
    (s / C.PROPUESTA).write_text('[areas]\na = ["a/**"]\n')
    (s / "sdlc.toml").write_text(antes.decode() + "# sucio\n")
    with pytest.raises(C.Rechazo, match="sucios"):
        C.areas(str(s), aprobar=True)
    (s / "sdlc.toml").write_bytes(antes)
    _git(s, "checkout", "-q", "-b", "otra")
    with pytest.raises(C.Rechazo, match="esperada main"):
        C.areas(str(s), aprobar=True)
    # 11. specs planas listadas y sus titulos en el prompt; 12. aviso sin-areas solo con specs planas
    prompts = []
    pl = _areas_repo(env / "pl", {"docs/specs/01-ingesta.md": "# Ingesta de datos\n"})
    assert "areas" in _avisos(C.doctor(str(pl))) and "areas" not in _avisos(C.doctor(str(_areas_repo(env / "np"))))
    C.areas(str(pl), llm=_llm('{"operaciones": []}', prompts))
    assert "Ingesta de datos" in prompts[0] and "docs/specs/01-ingesta.md" in (pl / C.PROPUESTA).read_text()
    # 17. raiz por nombres exactos y ningun archivo en dos areas
    rz = _areas_repo(env / "rz", {"motor_cuotas.py": "", "motor_fixture.py": ""})
    out = C.areas(str(rz), llm=_llm('{"operaciones": []}'))
    assert out["areas"]["raiz"] == ["motor_cuotas.py", "motor_fixture.py"]
    assert not [h for h in C._diagnostico(rz, {"areas": out["areas"]}) if h.costumbre == "areas"]
    # 18. nombre de carpeta en minusculas; choque de nombres aborta
    mc = _areas_repo(env / "mc", {"Motor_Cuotas/m.py": ""})
    assert "motor_cuotas" in C.areas(str(mc), llm=_llm('{"operaciones": []}'))["areas"]
    ch = _areas_repo(env / "ch", {"Ab/m.py": "", "ab/n.py": ""})
    with pytest.raises(C.Rechazo, match="mismo nombre"):
        C.areas(str(ch), llm=_llm('{"operaciones": []}'))
    # 12. archivo en dos areas declaradas: desvio de doctor
    dos = _conforme(env / "dos", files={"a/x.py": "", "sdlc.toml": SEPT_TOML + '[areas]\na = ["a/**"]\nt = ["**/*.py"]\n'})
    assert any("dos areas" in h["detalle"] for h in C.doctor(str(dos))["desvios"])
    # 13. doctor y --upgrade sin LLM (sin_llm activo)
    C.doctor(str(pl))
    C.upgrade(str(pl))
    # 19. la llamada real usa providers.call con nodo propio (providers.call registra en logs/metrics.jsonl)
    import mmorch.providers
    visto = {}

    def call(model, prompt, **kw):
        visto.update(kw, model=model)
        return type("R", (), {"text": '{"operaciones": []}'})()
    monkeypatch.setattr(mmorch.providers, "call", call)
    monkeypatch.setattr(C, "_llm_areas", LLM_REAL)
    C.areas(str(_areas_repo(env / "met")))
    assert visto["node"] == "sdlc-areas"


# ---------------------------------------------------------------------------------------------- ticket 09

class Fake:
    """Gestor de paquetes y docker simulados: registran cada comando y escriben lo que escribiria el real."""

    def __init__(self, falla: str = "", version: str = "6.100.0"):
        self.cmds, self.falla, self.version = [], falla, version

    def __call__(self, cmd, cwd, **kw):
        self.cmds.append((list(cmd), pathlib.Path(cwd)))
        texto = " ".join(map(str, cmd))
        if self.falla and self.falla in texto:
            return subprocess.CompletedProcess(cmd, 1, "", "fallo simulado")
        if cmd[:2] == ["npm", "install"]:
            pj = pathlib.Path(cwd) / "package.json"
            data = json.loads(pj.read_text())
            data.setdefault("devDependencies", {})["fast-check"] = "^3.0.0"
            pj.write_text(json.dumps(data))
            (pathlib.Path(cwd) / "package-lock.json").write_text("{}")
        if cmd[:2] == ["uv", "add"]:
            pp = pathlib.Path(cwd) / "pyproject.toml"
            pp.write_text(pp.read_text() + '\n[project.optional-dependencies]\ndev = ["hypothesis"]\n')
            (pathlib.Path(cwd) / "uv.lock").write_text("lock2\n")
        out = f"Name: hypothesis\nVersion: {self.version}\n" if "show" in cmd else ""
        return subprocess.CompletedProcess(cmd, 0, out, "")


PY = 'accept_cmd = "x"\nfiles = ["*.py"]\next = ["py"]\n'
TS = 'accept_cmd = "x"\nfiles = ["src/**/*.ts"]\next = ["ts"]\n'


def _prop(path, toml, extra=None, **kw):
    return _conforme(path, files={"sdlc.toml": toml, **(extra or {})}, **kw)


def test_libreria_de_propiedades(env):
    pyproj = '[project]\nname = "x"\n\n[project.optional-dependencies]\ndev = ["pytest>=8", "ruff"]\n'
    venv = {".venv/bin/python": ""}
    # 1. Python sin Hypothesis: aviso, nunca desvio; 2. TypeScript sin fast-check: aviso
    p1 = _prop(env / "p1", PY, {"pyproject.toml": pyproj})
    assert "propiedades" in _avisos(C.doctor(str(p1))) and "propiedades" not in _desvios(C.doctor(str(p1)))
    t1 = _prop(env / "t1", TS, {"package.json": "{}"})
    assert any("fast-check" in h["detalle"] for h in C.doctor(str(t1))["avisos"])
    # 3. declarado en extra dev, grupo o requirements*.txt: sin aviso; 4. en site-packages del .venv: sin aviso
    for i, extra in enumerate([{"pyproject.toml": pyproj.replace('"ruff"', '"ruff", "hypothesis>=6"')},
                               {"pyproject.toml": '[project]\nname = "x"\n[dependency-groups]\ndev = ["hypothesis"]\n'},
                               {"requirements-dev.txt": "hypothesis==6.1\n"},
                               {".venv/lib/python3.14/site-packages/hypothesis/__init__.py": ""}]):
        assert "propiedades" not in _avisos(C.doctor(str(_prop(env / f"d{i}", PY, extra)))), extra
    # 5. Java: sin aviso
    assert "propiedades" not in _avisos(C.doctor(str(_prop(env / "j", 'accept_cmd = "x"\nfiles = ["*.java"]\n'
                                                                     'ext = ["java"]\n'))))
    # 6. sin --aprobar: plan sin escrituras
    head = _head(p1)
    fake = Fake()
    out = C.propiedades(str(p1), run=fake)
    assert out["plan"] and not fake.cmds and _head(p1) == head and _git(p1, "status", "--porcelain") == ""
    # 7. con uv.lock corre uv add; 14. el commit incluye dependencias y lock
    u = _prop(env / "u", PY, {"pyproject.toml": '[project]\nname = "x"\n', "uv.lock": "lock\n"}, remoto=True)
    fake = Fake()
    out = C.propiedades(str(u), aprobar=True, run=fake)
    assert fake.cmds[0][0] == ["uv", "add", "--optional", "dev", "hypothesis"]
    assert set(_git(u, "show", "--name-only", "--format=", "HEAD").split()) == {"pyproject.toml", "uv.lock"}
    assert out["push"] == "ok"
    # 8. sin uv.lock: el extra dev suma hypothesis y el resto queda igual; 20. intérprete de _python_de
    p2 = _prop(env / "p2", PY + 'python = "envs/py"\n', {"pyproject.toml": pyproj, "envs/py": ""})
    fake = Fake()
    C.propiedades(str(p2), aprobar=True, run=fake)
    nuevo = (p2 / "pyproject.toml").read_text()
    assert nuevo == pyproj.replace('"ruff"]', '"ruff", "hypothesis"]')
    assert "hypothesis" in C.tomllib.loads(nuevo)["project"]["optional-dependencies"]["dev"]
    assert fake.cmds[0][0][:4] == [str(p2 / "envs/py"), "-m", "pip", "install"]
    assert P._python_de(p2, {"python": "envs/py"}) == str(p2 / "envs/py")
    # 9. requirements.txt sin salto final: linea al final; 17. con == si el archivo pinea; 18. Adepor
    rq = _prop(env / "rq", PY, {"requirements.txt": "# deps\nrequests==2.34.2", "requirements-analisis.txt": "x\n",
                                **venv})
    C.propiedades(str(rq), aprobar=True, run=Fake())
    assert (rq / "requirements.txt").read_text() == "# deps\nrequests==2.34.2\nhypothesis==6.100.0\n"
    assert (rq / "requirements-analisis.txt").read_text() == "x\n"
    # 13. sin .venv: declarada, pip no corre y aviso
    nv = _prop(env / "nv", PY, {"requirements.txt": "requests\n"})
    fake = Fake()
    out = C.propiedades(str(nv), aprobar=True, run=fake)
    assert not fake.cmds and (nv / "requirements.txt").read_text() == "requests\nhypothesis\n"
    assert any("sin .venv" in a for a in out["avisos"])
    # 10. package.json en app/: npm corre en app/
    es = _prop(env / "es", TS, {"app/package.json": "{}", "app/src/a.ts": ""})
    fake = Fake()
    C.propiedades(str(es), aprobar=True, run=fake)
    assert fake.cmds[0] == (["npm", "install", "-D", "fast-check"], es / "app")
    # 11. contenedor: bloque con marcas en Dockerfile.sdlc y build con el comando del builder
    f2 = _prop(env / "f2", TS + 'contenedor = "f2-sdlc"\n', {"package.json": "{}", "Dockerfile.sdlc": "FROM node\n"})
    fake = Fake()
    C.propiedades(str(f2), aprobar=True, run=fake)
    assert (f2 / "Dockerfile.sdlc").read_text().startswith("FROM node\n\n# BEGIN SDLC propiedades v:1 hash:")
    assert fake.cmds[-1][0][1:] == ["build", "-q", "-f", "Dockerfile.sdlc", "-t", "f2-sdlc", "."]
    assert set(_git(f2, "show", "--name-only", "--format=", "HEAD").split()) == {"package.json", "package-lock.json",
                                                                                  "Dockerfile.sdlc"}
    assert "propiedades" not in _avisos(C.doctor(str(f2))) and "dockerfile" not in _avisos(C.doctor(str(f2)))
    # 19. segunda corrida con la libreria presente: ni escritura ni commit
    head = _head(f2)
    assert C.propiedades(str(f2), aprobar=True, run=Fake())["hecho"] == [] and _head(f2) == head
    # 12. bloque editado en Dockerfile.sdlc: aviso con diff e intacto
    editado = "FROM node\n" + C.bloque("propiedades", "Dockerfile.sdlc").replace("fast-check\n", "fast-check@1\n")
    f3 = _prop(env / "f3", TS + 'contenedor = "f2-sdlc"\n', {"package.json": "{}", "Dockerfile.sdlc": editado})
    assert any("editado" in h["detalle"] for h in C.doctor(str(f3))["avisos"] if h["ruta"] == "Dockerfile.sdlc")
    out = C.propiedades(str(f3), aprobar=True, run=Fake())
    assert (f3 / "Dockerfile.sdlc").read_text() == editado and any("no se toca" in a for a in out["avisos"])
    # 15. el gestor falla: bytes originales y sin commit; 16. el build falla: Dockerfile.sdlc vuelve a sus bytes
    for i, falla in enumerate(["npm install", "build"]):
        rr = _prop(env / f"x{i}", TS + 'contenedor = "f2-sdlc"\n', {"package.json": "{}", "Dockerfile.sdlc": "FROM n\n"})
        head = _head(rr)
        with pytest.raises(C.Rechazo, match="fallo|no se construyo"):
            C.propiedades(str(rr), aprobar=True, run=Fake(falla=falla))
        assert (rr / "package.json").read_text() == "{}" and (rr / "Dockerfile.sdlc").read_text() == "FROM n\n"
        assert not (rr / "package-lock.json").exists() and _head(rr) == head
    # 14. fuera de la rama default aborta, como en el ticket 07
    rb = _prop(env / "rb", PY, {"requirements.txt": "x\n"})
    _git(rb, "checkout", "-q", "-b", "otra")
    with pytest.raises(C.Rechazo, match="esperada main"):
        C.propiedades(str(rb), aprobar=True, run=Fake())
    assert os.path.exists(rb / "requirements.txt")


def test_init_techo_sin_comodin_en_la_raiz(env):
    """`init` lista por nombre exacto los archivos sueltos de la raiz: `*.py` calza con todo el repo en `_calza()`."""
    repo = _repo(env / "techo", {"motor.py": "", "util.py": "", "pkg/a.py": "", "tests/test_x.py": "", "README.md": ""})
    P.init(str(repo))
    files = C.tomllib.loads((repo / "sdlc.toml").read_text(encoding="utf-8"))["files"]
    assert files == ["motor.py", "pkg/**/*.py", "util.py"]
    assert not any(P._calza("tests/test_x.py", g) for g in files)
    assert all(any(P._calza(f, g) for g in files) for f in ("motor.py", "pkg/a.py", "util.py"))
