"""SDD fase 2, rama de propiedades (tickets 07 a 10 de .scratch/sdd-fase-2/): marca `[prop]`, tests de propiedades en la
etapa 1, propiedad vacua sobre las corridas de gate_mutacion, semilla del sha base y JUnit como unico parser. Cada test es
el aprobado del ticket; `# n.` son sus casos. Hypothesis corre de verdad (venv de orchestration); fast-check es un stub
local en node_modules (sin red)."""
import json
import shutil
import subprocess

import pytest

import sdlc.areas as A
import sdlc.pipeline as S
import sdlc.propiedades as P
import sdlc_juguete as J

pytest.importorskip("hypothesis")

SPEC4 = "## Requisitos\n\n- R1: a.\n- R2 [prop]: b.\n- R3 [prop]: c.\n- R4: d; un texto que menciona [prop] adentro.\n"


# ---------------------------------------------------------------- ticket 07


def test_detecta_marcas(tmp_path, monkeypatch):
    # 1. exactamente los dos IDs marcados; 2. un texto que menciona [prop] dentro de otra regla no marca
    assert P.marcas(SPEC4) == ["R2", "R3"]
    # 3. una marca mal escrita no cuenta
    assert P.marcas("## Requisitos\n\n- R5 [Prop]: x.\n- R6 [props]: y.\n") == []
    # 4. una spec sin marcas devuelve una lista vacia (la corrida sigue igual: ver test_etapa_de_propiedades)
    assert P.marcas("## Requisitos\n\n- R1: a.\n") == []
    # 5. un MODIFIED que quita la marca deja el requisito sin marca en la spec fusionada
    base = J.spec_area({"R2 [prop]": "b.", "R3 [prop]": "c."})
    assert P.marcas(A.fusionar(base, "## MODIFIED\n\n- R2: b.\n")) == ["R3"]
    # 9. un ADDED con marca deja el requisito marcado en la spec fusionada
    assert P.marcas(A.fusionar(base, "## ADDED\n\n- R1 [prop]: a.\n")) == ["R1", "R2", "R3"]
    # 10. un MODIFIED que solo agrega o quita la marca pasa el gate del ticket 04
    assert A.gate_delta(base, "## MODIFIED\n\n- R2: b.\n", {"R2": ["t.py::test_R2_x"]}) == []
    # 6. la deteccion funciona igual en la spec de un area y en docs/sdlc/spec.md
    feature = "# Spec: x\n\n## Requisitos\n\n- R1: a.\n- R3 [prop]: c.\n\n## Trazabilidad\n\n| ID | tests |\n|---|---|\n"
    assert P.marcas(base) == ["R2", "R3"] and P.marcas(feature) == ["R3"]
    # 7. gate_traza_spec sigue encontrando R3 cuando R3 lleva la marca
    J.conf(J.repo(tmp_path), monkeypatch, files=[])
    ok, note = S.gate_traza_spec(feature + "| R1 | test_R1_a |\n| R3 | test_R3_prop_b |\n", {"test_R1_a", "test_R3_prop_b"})
    assert ok, note
    assert not S.gate_traza_spec(feature + "| R1 | test_R1_a |\n", {"test_R1_a"})[0]
    # 8. la plantilla incluye el criterio de la marca
    tpl = (S.TPL / "spec-template.md").read_text(encoding="utf-8")
    assert "[prop]" in tpl and "conjunto de entradas generable" in tpl


def test_marcas_en_la_spec_base(tmp_path, monkeypatch):
    # 11 (ticket 07). la spec base que escribe la etapa 0 tambien puede llevar marcas
    repo = J.repo(tmp_path)
    char = "tests/sdlc/calc/test_R1_base.py"
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {char: CHAR_PROP}, {"tests/sdlc/calc/test_R2_resta.py": ACEPT}))
    S.run(1, 1)
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    monkeypatch.setattr(S, "llm", lambda m, s, u, timeout=400: "- R1 [prop]: suma(a, b) == suma(b, a)." if "requisitos base" in u
                        else "## ADDED\n\n- R2: resta(a, b) devuelve a - b.\n")
    S.run(2, 2)
    assert P.marcas((repo / "docs/specs/calc/spec.md").read_text(encoding="utf-8")) == ["R1"]


# ---------------------------------------------------------------- ticket 08

CHAR_PROP = """from hypothesis import given, strategies as st

from pkg.a import suma


def test_R1_suma():
    assert suma(1, 2) == 3


@given(st.integers(), st.integers())
def test_R1_prop_conmuta(a, b):
    assert suma(a, b) == suma(b, a)
"""

ACEPT = "def test_R2_resta():\n    from pkg.a import resta\n    assert resta(3, 1) == 2\n"

R3 = """from hypothesis import given, strategies as st

from pkg.c import doble


def test_R3_triple():
    from pkg.c import triple
    assert triple(2) == 6


@given(st.integers())
def test_R3_prop_doble_par(x):
    assert doble(x) % 2 == 0
"""
R3_REL = "tests/sdlc/calc/test_R3_triple.py"
R3_SOLO_PROP = R3.split("def test_R3_triple")[0] + R3.split("    assert triple(2) == 6\n")[1].lstrip("\n")


def _etapa1(repo, monkeypatch, archivos, approve=False, files=("pkg/c.py",)):
    J.conf(repo, monkeypatch, files=list(files), approve=approve)
    S._resolver_area()
    S.state["base_sha"] = J.git(repo, "rev-parse", "--short", "HEAD").strip()
    S._preparar_propiedades()
    claude = J.Claude(repo, archivos)
    monkeypatch.setattr(S, "_revisor", claude)
    return claude


def test_etapa_de_propiedades(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    # 1. la etapa 1 deja un test_R<n>_prop_* con @given en el archivo del ID; 6. la propiedad verde en HEAD pasa
    #    porque R3 tiene otro test rojo
    claude = _etapa1(repo, monkeypatch, {R3_REL: R3})
    S.aceptacion()
    assert S.state["ids_con_prop"] == ["R3"] and "Hypothesis" in claude.prompts[0]
    # 10. accept() corre los tests de propiedades como parte de la aceptacion
    J.escribir(repo, {"pkg/c.py": J.C + "\n\ndef triple(x):\n    return 3 * x\n"})
    assert S.accept()[0]
    J.escribir(repo, {"pkg/c.py": J.C.replace("2 * x", "2 * x + 1") + "\n\ndef triple(x):\n    return 3 * x\n"})
    ok, log = S.accept()
    assert not ok and "test_R3_prop_doble_par" in log
    J.git(repo, "checkout", "--", "pkg/c.py")
    # 9 y 13. ningun prompt por API incluye el codigo de las propiedades, si la salida del fallo
    assert "doble(x) % 2" not in S._tests_text() and "test_R3_triple" in S._tests_text()
    prompts = []
    monkeypatch.setattr(S, "llm", lambda m, s, u, timeout=400: prompts.append(u) or '{"files": [], "instructions": {}}')
    S.state["plan_files"] = ["pkg/c.py"]
    S.reasoner_rounds("FAILED test_R3_prop_doble_par - assert 3 % 2 == 0")
    assert prompts and all("doble(x) % 2" not in p for p in prompts) and "assert 3 % 2 == 0" in prompts[0]
    J.escribir(repo, {"docs/sdlc/spec.md": "spec\n", "docs/sdlc/plan.md": _plan_c()})
    monkeypatch.setattr(S, "llm", lambda m, s, u, timeout=400: prompts.append(u) or J.C)
    monkeypatch.setattr(S, "gate_compile", lambda files: (False, "corta"))
    monkeypatch.setattr(S, "_compile_ladder", lambda log, c: (False, "corta", log))
    S.state["plan_files"] = ["pkg/c.py"]
    with pytest.raises(S.StageFailed):
        S.build()
    assert all("doble(x) % 2" not in p for p in prompts)
    for rel in list(S.TASK.accept_files):
        (repo / rel).unlink()
    J.git(repo, "reset", "-q", "--hard", "HEAD~1")
    # 2. un test de propiedades sin @given corta la etapa 1 y el gate nombra el motivo
    _etapa1(repo, monkeypatch, {R3_REL: R3.replace("@given(st.integers())\n", "")})
    with pytest.raises(S.StageFailed, match="@given"):
        S.aceptacion()
    (repo / R3_REL).unlink()
    # 3. un test de propiedades sin el ID en el nombre tambien corta
    _etapa1(repo, monkeypatch, {R3_REL: R3.replace("test_R3_prop_doble_par", "test_prop_doble_par")})
    with pytest.raises(S.StageFailed, match="sin ID"):
        S.aceptacion()
    (repo / R3_REL).unlink()
    # 7. un ID ADDED con todos sus tests verdes en HEAD corta la etapa 1
    _etapa1(repo, monkeypatch, {R3_REL: R3_SOLO_PROP})
    with pytest.raises(S.StageFailed, match="R3"):
        S.aceptacion()
    (repo / R3_REL).unlink()
    # 8. la parada unica lista los tests de propiedades junto con los demas
    _etapa1(repo, monkeypatch, {R3_REL: R3}, approve=True)
    with pytest.raises(S.StageFailed, match="aprobacion"):
        S.aceptacion()
    sup = (repo / "docs/sdlc/supervision.md").read_text(encoding="utf-8")
    assert "test_R3_prop_doble_par" in sup and "test_R3_triple" in sup


def _plan_c():
    return "## Archivos\n- `pkg/c.py` [R3]\n\n## Prueba\npytest\n"


def test_gate_de_marcas(tmp_path, monkeypatch):
    repo = J.repo(tmp_path, toml='accept_cmd = "python -m pytest -q"\next = ["py"]\nfiles = ["pkg/*.py"]\n')
    J.conf(repo, monkeypatch, files=["pkg/c.py"])
    S.state["ids_con_prop"] = ["R3"]
    # 4. una marca sin test de propiedades hace fallar el gate de la etapa 2
    assert not S.gate_marcas("## Requisitos\n\n- R1 [prop]: a.\n- R3 [prop]: c.\n")[0]
    # 5. un test de propiedades sin marca tambien
    ok, note = S.gate_marcas("## Requisitos\n\n- R3: c.\n")
    assert not ok and "R3" in note
    assert S.gate_marcas("## Requisitos\n\n- R3 [prop]: c.\n")[0]
    # 13 (ticket 10). sin libreria (ninguna propiedad), el gate de marcas pasa sin marcas
    S.state["ids_con_prop"] = []
    assert S.gate_marcas("## Requisitos\n\n- R3: c.\n")[0]


def test_propiedades_sin_area(tmp_path, monkeypatch):
    # 11 (ticket 08). sin area, la propiedad vive en el archivo de aceptacion de hoy, con las mismas reglas
    toml = 'accept_cmd = "python -m pytest -q"\next = ["py"]\nfiles = ["pkg/*.py"]\napprove_accept = false\n'
    repo = J.repo(tmp_path, toml=toml)
    claude = _etapa1(repo, monkeypatch, {"tests/test_sdlc_feat_x.py": R3.replace("@given(st.integers())\n", "")})
    with pytest.raises(S.StageFailed, match="@given"):
        S.aceptacion()
    assert "tests/test_sdlc_feat_x.py" in claude.prompts[0]
    _etapa1(repo, monkeypatch, {"tests/test_sdlc_feat_x.py": R3})
    S.aceptacion()
    assert S.state["ids_con_prop"] == ["R3"]


def test_propiedad_de_caracterizacion_pasa_en_head(tmp_path, monkeypatch):
    # 12 (ticket 08). una propiedad de la etapa 0 debe pasar en HEAD, como todo test de caracterizacion
    repo = J.repo(tmp_path)
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    rota = CHAR_PROP.replace("assert suma(a, b) == suma(b, a)", "assert suma(a, b) == a")
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {"tests/sdlc/calc/test_R1_base.py": rota}))
    with pytest.raises(S.StageFailed, match="test_R1_prop_conmuta"):
        S.caracterizar()


# ---------------------------------------------------------------- ticket 09

M = "def clamp(x):\n    if x > 10:\n        return 10\n    return x\n"
VACUA = """from hypothesis import given, strategies as st

from pkg.m import clamp


def test_R1_ejemplo():
    assert clamp(3) == 3


@given(st.integers())
def test_R1_prop_tope(x):
    assert clamp(x) <= 10


@given(st.integers())
def test_R1_prop_entero(x):
    assert isinstance(clamp(x), int)
"""


def _mutacion(tmp_path, monkeypatch, minimo=None):
    toml = 'accept_cmd = "python -m pytest -q"\next = ["py"]\nfiles = ["pkg/*.py"]\n'
    if minimo is not None:
        toml += f"mutation_min = {minimo}\n"
    repo = J.repo(tmp_path, toml=toml, archivos={"tests/test_sdlc_clamp.py": VACUA})
    J.conf(repo, monkeypatch, files=["pkg/m.py"], accept={"tests/test_sdlc_clamp.py": VACUA})
    S.state["base_sha"] = J.git(repo, "rev-parse", "--short", "HEAD").strip()
    S._preparar_propiedades()
    J.escribir(repo, {"pkg/m.py": M})   # archivo nuevo: todas sus lineas mutan
    S.state["plan_files"] = ["pkg/m.py"]
    return repo


def test_propiedad_vacua_detectada(tmp_path, monkeypatch):
    _mutacion(tmp_path, monkeypatch, minimo=0.1)
    corridas = []
    real = S.sh
    monkeypatch.setattr(S, "sh", lambda cmd, *a, **k: (corridas.append(cmd), real(cmd, *a, **k))[1])
    ok, note = S.gate_mutacion()
    muts = S.state["mutantes"]
    # 3. una corrida de la aceptacion por mutante, como hoy
    assert len([c for c in corridas if "pytest" in str(c)]) == len(muts) == 3
    # 1. la propiedad del tope mata un mutante que el ejemplo no mata
    assert any("test_R1_prop_tope" in m["fallaron"] and "test_R1_ejemplo" not in m["fallaron"] for m in muts)
    # 10. un mutante que mata primero el ejemplo igual registra la propiedad (sin -x)
    assert any({"test_R1_ejemplo", "test_R1_prop_tope"} <= set(m["fallaron"]) for m in muts)
    # 2. la otra propiedad no mata ningun mutante y el run-log la nombra vacua; 8. el ejemplo nunca queda vacuo
    assert S.state["propiedades_vacuas"] == ["test_R1_prop_entero"]
    # 5. con mutation_min, la propiedad vacua bloquea y el mensaje la nombra
    assert not ok and "test_R1_prop_entero" in note
    # 9. mutation_score se calcula como hoy: muertos / total
    assert S.state["mutation_score"] == round(sum(1 for m in muts if m["fallaron"]) / len(muts), 3)


def test_propiedad_vacua_sin_minimo(tmp_path, monkeypatch):
    # 4. sin mutation_min, la propiedad vacua queda anotada y el gate pasa
    _mutacion(tmp_path, monkeypatch)
    ok, _ = S.gate_mutacion()
    assert ok and S.state["propiedades_vacuas"] == ["test_R1_prop_entero"]


CUENTA = """import pathlib

from hypothesis import given, strategies as st


@given(st.integers())
def test_R1_prop_cuenta(x):
    with open(pathlib.Path(__file__).parent / "cuenta.txt", "a") as f:
        f.write("x")
"""


def test_perfil_de_ejemplos(tmp_path, monkeypatch):
    repo = _mutacion(tmp_path, monkeypatch)
    J.escribir(repo, {"tests/test_sdlc_cuenta.py": CUENTA})
    S.TASK.accept_files["tests/test_sdlc_cuenta.py"] = CUENTA
    cuenta = repo / "tests/cuenta.txt"
    # 7. la aceptacion normal usa el numero de ejemplos por defecto (100)
    S.accept()
    assert len(cuenta.read_text()) == 100
    cuenta.unlink()
    # 6. dentro de los mutantes, el perfil de 20 ejemplos
    (repo / "pkg/m.py").write_text(M.replace("return 10", "return 11"), encoding="utf-8")
    S._accept_mata()
    assert len(cuenta.read_text()) == 20


def test_mutacion_sin_atribucion(tmp_path, monkeypatch):
    # 11 (ticket 09). con accept_cmd de un runner sin JUnit, el run-log avisa y el gate queda como hoy
    repo = J.repo(tmp_path, toml='accept_cmd = "python check.py"\next = ["py"]\nfiles = ["pkg/*.py"]\n',
                  archivos={"check.py": "import pkg.m as m\nassert m.clamp(3) == 3\n"})
    J.conf(repo, monkeypatch, files=["pkg/m.py"])
    S.ACCEPT_CMD = S.CFG["accept_cmd"]
    J.escribir(repo, {"pkg/m.py": M})
    S.state["plan_files"] = ["pkg/m.py"]
    ok, _ = S.gate_mutacion()
    assert ok and "sin atribucion por test" in S.state["atribucion"]


# ---------------------------------------------------------------- ticket 10

RARA = """from hypothesis import given, strategies as st


@given(st.integers(0, 10 ** 6))
def test_R1_prop_rara(x):
    assert x % 1000 != 777
"""


def _py(tmp_path, monkeypatch, archivos=None, toml='accept_cmd = "python -m pytest -q"\next = ["py"]\nfiles = ["pkg/*.py"]\n'):
    repo = J.repo(tmp_path, toml=toml, archivos=archivos)
    J.conf(repo, monkeypatch, files=["pkg/a.py"], accept={k: v for k, v in (archivos or {}).items() if k.startswith("tests/test_sdlc")})
    S.state["base_sha"] = J.git(repo, "rev-parse", "--short", "HEAD").strip()
    S._preparar_propiedades()
    return repo


def test_propiedad_determinista(tmp_path, monkeypatch):
    repo = _py(tmp_path, monkeypatch, {"tests/test_sdlc_rara.py": RARA})
    # 1. el gate corre dos veces sobre el mismo codigo y da el mismo veredicto
    assert S.accept()[0] == S.accept()[0]
    # 3. el mismo commit base siempre produce la misma semilla; 2. dos commits distintos, semillas distintas, anotadas
    sem = S.state["propiedades"]["semilla"]
    assert sem == P.semilla(S.state["base_sha"]) and P.semilla("abc1234") != P.semilla("def5678")
    S._flush()
    assert json.loads((repo / "docs/sdlc/run-log.json").read_text(encoding="utf-8"))["propiedades"]["semilla"] == sem
    # 8. el archivo auxiliar no aparece en git status ni entra a ningun commit
    assert (repo / "_sdlc_prop.py").exists() and "_sdlc_prop" not in J.git(repo, "status", "--porcelain")
    J.git(repo, "add", "-A")
    assert "_sdlc_prop" not in J.git(repo, "diff", "--cached", "--name-only")
    # 11. una corrida no deja la carpeta .hypothesis/ en el worktree
    assert not (repo / ".hypothesis").exists()
    # 9. el perfil de 20 ejemplos se activa solo dentro de los mutantes
    assert "max_examples=20" in (repo / "_sdlc_prop_mut.py").read_text() and "max_examples" not in (repo / "_sdlc_prop.py").read_text()


def test_sin_libreria(tmp_path, monkeypatch):
    # 4. un repo Python sin Hypothesis sigue sin propiedades y el run-log anota la falta
    import sys
    sin_hyp = shutil.which("python3", path="/usr/bin")
    if not sin_hyp or subprocess.run([sin_hyp, "-c", "import hypothesis"], capture_output=True).returncode == 0:
        pytest.skip("no hay un Python sin Hypothesis para simular el repo")
    toml = f'accept_cmd = "python -m pytest -q"\next = ["py"]\nfiles = ["pkg/*.py"]\npython = "{sin_hyp}"\napprove_accept = false\n'
    repo = _py(tmp_path, monkeypatch, toml=toml)
    assert S.PY != sys.executable
    assert S.state["propiedades"]["libreria"] is None and "hypothesis" in S.state["propiedades"]["nota"]
    # 13. sin libreria, el prompt de la etapa 1 no pide propiedades
    S.PY = sys.executable  # el resto de la etapa corre con un pytest que existe
    claude = J.Claude(repo, {"tests/test_sdlc_feat_x.py": ACEPT.replace("resta", "suma").replace("R2", "R1")})
    monkeypatch.setattr(S, "_revisor", claude)
    try:
        S.aceptacion()
    except S.StageFailed:
        pass
    assert "Hypothesis" not in claude.prompts[0] and "propiedad" not in claude.prompts[0].lower()


def _ts(tmp_path, monkeypatch, *, fast_check=True, toml_extra=""):
    toml = f'accept_cmd = "node --test"\next = ["mjs"]\nfiles = ["src/*.mjs"]\napprove_accept = false\n{toml_extra}'
    archivos = {"src/m.mjs": "export const doble = (x) => 2 * x;\n", ".gitignore": "node_modules/\n"}
    if fast_check:
        archivos |= {"node_modules/fast-check/package.json": json.dumps({"name": "fast-check", "type": "module", "exports": "./index.js"}),
                     "node_modules/fast-check/index.js": "export function configureGlobal(c) { globalThis.__fc = c; }\n"
                                                         "export default { configureGlobal };\n"}
    repo = J.repo(tmp_path, toml=toml, archivos=archivos)
    t = J.conf(repo, monkeypatch, files=["src/m.mjs"], name="feat-x")  # el nombre del contenedor no admite espacios
    S.ACCEPT_CMD = "node --test"
    S.state["base_sha"] = J.git(repo, "rev-parse", "--short", "HEAD").strip()
    S._preparar_propiedades()
    return repo, t


TS_CFG = """import test from 'node:test';
import assert from 'node:assert';

test('test_R1_semilla', () => { assert.equal(globalThis.__fc?.seed, SEMILLA); });
test('test_R1_prop_mal', () => { assert.equal(1, 2); });
"""


def test_typescript_con_fast_check(tmp_path, monkeypatch):
    # 6. un repo TypeScript con fast-check y node --test recibe propiedades y atribucion por JUnit
    repo, t = _ts(tmp_path, monkeypatch)
    assert S.state["propiedades"]["libreria"] == "fast-check"
    J.escribir(repo, {"tests/m.test.mjs": TS_CFG.replace("SEMILLA", str(S.state["propiedades"]["semilla"]))})
    ok, log, res = S._corre_con_junit([])
    assert not ok and res == {"test_R1_semilla": True, "test_R1_prop_mal": False}
    claude = J.Claude(repo, None)
    monkeypatch.setattr(S, "_revisor", claude)
    with pytest.raises(S.StageFailed):
        S.aceptacion()
    assert "fast-check" in claude.prompts[0]
    # 7. el parser de JUnit lee igual la salida de pytest y la de node --test
    py = J.repo(tmp_path / "py", toml='accept_cmd = "x"\nfiles = ["pkg/*.py"]\n',
                archivos={"tests/test_sdlc_j.py": "def test_R1_ok():\n    assert True\n\n\ndef test_R1_prop_mal():\n    assert False\n"})
    xml = tmp_path / "py.xml"
    subprocess.run([S.PY, "-m", "pytest", "tests/test_sdlc_j.py", "-q", f"--junitxml={xml}", "-p", "no:cacheprovider"], cwd=py)
    assert P.junit(xml) == {"test_R1_ok": True, "test_R1_prop_mal": False}


def test_typescript_sin_fast_check(tmp_path, monkeypatch):
    # 5. un repo TypeScript sin fast-check sigue igual, con la misma anotacion
    _ts(tmp_path, monkeypatch, fast_check=False)
    assert S.state["propiedades"]["libreria"] is None and "fast-check" in S.state["propiedades"]["nota"]


def test_runner_sin_junit(tmp_path, monkeypatch):
    # 10. un runner sin salida JUnit sigue sin atribucion
    assert P.con_junit("mvn -q test", "/tmp/x.xml") is None and P.con_junit("cargo test", "/tmp/x.xml") is None
    assert "--test-reporter=junit" in P.con_junit("node --experimental-strip-types --test", "/tmp/x.xml")
    assert "--junitxml=/tmp/x.xml" in P.con_junit("python -m pytest -q", "/tmp/x.xml")


def _imagen_node():
    from mmorch.sandbox import engine
    eng = engine()
    if not eng:
        return None
    r = subprocess.run([eng, "image", "exists", "docker.io/library/node:22-slim"], capture_output=True)
    return eng if r.returncode == 0 or eng.endswith("docker") else None


@pytest.mark.skipif(not _imagen_node(), reason="sin podman/docker o sin la imagen node:22-slim local")
def test_auxiliar_en_el_contenedor(tmp_path, monkeypatch):
    # 12. dentro del contenedor sin red, el archivo auxiliar carga igual que en el host
    repo, t = _ts(tmp_path, monkeypatch, toml_extra='contenedor = "docker.io/library/node:22-slim"\n')
    J.escribir(repo, {"tests/m.test.mjs": TS_CFG.replace("SEMILLA", str(S.state["propiedades"]["semilla"]))})
    ok, log, res = S._corre_con_junit([])
    assert res and res["test_R1_semilla"] is True, log
