"""Gates puros de mmorch.sdlc (ticket 05, paso 1). Cero API, cero git: configure() con una task ficticia.

Cada caso viene de un defecto real del driver (research/07-resultados.md del mapa sdlc-6-gates):
- strip_fence: una valla ``` DENTRO de un string cortaba el archivo (D2, D11-diag2).
- one_file: el coder pego 3 archivos con cabeceras `# path` en una respuesta (r1 2026-09-11).
- plan-allowlist: tests/ prohibido salvo los que la feature exige (D14: test_capas.py del ratchet).
- cambio-minimo: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda (D11).
- clarificaciones: Claude escribe "0 preguntas" y "afecta: Contrato" con formato libre (D11-diag).
"""
import subprocess
import sys
import types

import pytest

import mmorch.sdlc as S

INNER = 'x = "```\\n{cur}\\n```"\ny = 1'


@pytest.fixture
def run(tmp_path):
    t = types.SimpleNamespace(name="t", task="agrega una guarda; no se tocan tests", accept_files={})
    feat = {"repo": str(tmp_path), "files": ["pkg/a.py", "tests/test_capas.py"], "suite": ["tests"]}
    S.configure(t, contract=["a"], feat=feat, wt=tmp_path / "wt")
    return S


def test_strip_fence_solo_vallas_de_linea_entera(run):
    assert S.strip_fence("Aca va:\n```python\n" + INNER + "\n```\nlisto") == INNER
    assert S.strip_fence("```python\n" + INNER + "\n```") == INNER
    assert S.strip_fence(INNER) == INNER


def test_one_file_recorta_su_seccion(run):
    pegado = "# pkg/a.py\nA = 1\n# pkg/b.py\nB = 2\n"
    assert S.one_file(pegado, "pkg/a.py").strip() == "A = 1"
    assert S.one_file("solo = 1\n", "pkg/a.py") == "solo = 1\n"


def test_plan_allowlist_tests_ajenos_no_y_obligatorios_si(run):
    ok, _ = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_capas.py` [R1]\n", ["pkg/a.py", "tests/test_capas.py"])
    assert ok
    ok, nota = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_otro.py` [R1]\n", ["pkg/a.py"])
    assert not ok and "test_otro" in nota
    ok, nota = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n", ["pkg/a.py"])
    assert not ok and "omite" in nota


def test_plan_files_conserva_los_tests_obligatorios(run):
    assert S._plan_files("- `pkg/a.py`\n- `tests/test_capas.py`\n- `tests/test_otro.py`\n") == ["pkg/a.py", "tests/test_capas.py"]


def test_cambio_minimo_rechaza_reescritura_y_acepta_parche(run):
    orig = "\n".join(f"linea_{i} = {i}" for i in range(100)) + "\n"
    S._orig_files.clear(); S._orig_files["pkg/a.py"] = orig
    ok, nota = S.gate_cambio_minimo({}, {"pkg/a.py": "nuevo = 1\n"})
    assert not ok and "100 de 100" in nota
    ok, _ = S.gate_cambio_minimo({}, {"pkg/a.py": orig + "guarda = True\n"})
    assert ok


def test_cambio_minimo_pasa_si_la_tarea_pide_reescribir(run):
    S.TASK.task = "reescribi pkg/a.py entero"
    S._orig_files.clear(); S._orig_files["pkg/a.py"] = "a = 1\n" * 50
    assert S.gate_cambio_minimo({}, {"pkg/a.py": "b = 2\n"})[0]


def test_clarificaciones_acepta_cero_y_afecta_libre(run):
    assert S.gate_clarificaciones("## Clarificaciones\n\nHice 0 preguntas: la spec cubre todo.\n")[0]
    ok, nota = S.gate_clarificaciones("## Clarificaciones\n- P1: ¿x? | R: si | afecta: Contrato\n")
    assert ok and "1 preguntas" in nota
    assert not S.gate_clarificaciones("## Otra\n")[0]
    assert not S.gate_clarificaciones("## Clarificaciones\n\n")[0]


def test_files_from_toml_es_el_techo(tmp_path):
    with pytest.raises(ValueError):
        S._files_from_toml(str(tmp_path))
    (tmp_path / "sdlc.toml").write_text('files = ["pkg/a.py"]\n', encoding="utf-8")
    assert S._files_from_toml(str(tmp_path)) == ["pkg/a.py"]


def test_resolve_files_toml_es_techo_y_el_payload_solo_acota(tmp_path):
    with pytest.raises(ValueError):
        S._resolve_files(str(tmp_path), None)
    assert S._resolve_files(str(tmp_path), ["pkg/a.py"]) == ["pkg/a.py"]  # sin toml: lo que dice el llamador
    (tmp_path / "sdlc.toml").write_text('files = ["pkg/a.py", "pkg/b.py"]\n', encoding="utf-8")
    assert S._resolve_files(str(tmp_path), None) == ["pkg/a.py", "pkg/b.py"]
    assert S._resolve_files(str(tmp_path), ["pkg/b.py"]) == ["pkg/a.py", "pkg/b.py"]  # devuelve el techo; b es obligatorio
    with pytest.raises(ValueError, match="fuera del techo"):
        S._resolve_files(str(tmp_path), ["pkg/c.py"])
    (tmp_path / "sdlc.toml").write_text('files = ["pkg/**/*.py", "tests/test_capas.py"]\n', encoding="utf-8")
    assert S._resolve_files(str(tmp_path), ["pkg/sub/nuevo.py"]) == ["pkg/**/*.py", "tests/test_capas.py"]  # glob


def test_techo_con_globs_y_varios_lenguajes(run):
    S.FEAT["techo"] = ["src/**/*.java", "pkg/*.py"]
    S.CFG["ext"] = ["py", "java"]
    assert S._en_techo("src/main/App.java") and S._en_techo("pkg/a.py") and not S._en_techo("otro/x.py")
    assert S._calza("src/App.java", "src/**/*.java")  # init escribe d/**/*.ext: incluye los archivos del nivel d
    assert S._plan_files("- `src/main/App.java` [R1]\n- `pkg/a.py` [R2]\n- `otro/x.rb`\n") == ["src/main/App.java", "pkg/a.py"]
    plan = "## Archivos\n- `pkg/a.py` [R1]\n- `tests/test_capas.py` [R1]\n- `otro/x.py` [R1]\n"
    ok, nota = S.gate_plan_allowlist(plan, ["pkg/a.py", "tests/test_capas.py", "otro/x.py"])
    assert not ok and "fuera del techo" in nota
    assert S.one_file("// src/A.java\nclass A {}\n// src/B.java\nclass B {}\n", "src/B.java").strip() == "class B {}"


def test_compile_y_lint_por_comando_para_otros_lenguajes(run, tmp_path):
    (S.WT / "src").mkdir(parents=True, exist_ok=True)
    (S.WT / "src" / "A.java").write_text("class A {}\n", encoding="utf-8")
    S.CFG["compile_cmd"] = "python -c \"import sys; sys.exit(0)\""
    assert S.gate_compile(["src/A.java"])[0]
    S.CFG["compile_cmd"] = "python -c \"import sys; sys.exit(1)\""
    assert not S.gate_compile(["src/A.java"])[0]
    S.state["plan_files"] = ["src/A.java"]
    S.CFG["lint_cmd"] = "python -c \"import sys; sys.exit(0)\""
    assert S.gate_lint()[0] and S.state["lint_new"] == {"lint_cmd": 0}
    # {files}: el comando recibe solo los archivos del plan (lint de repo con errores previos, node --check por archivo)
    (S.WT / "chk.py").write_text("import sys; sys.exit(0 if sys.argv[1:] == ['src/A.java'] else 1)", encoding="utf-8")
    S.CFG["lint_cmd"] = S.CFG["compile_cmd"] = "python chk.py {files}"
    assert S.gate_lint()[0] and S.gate_compile(["src/A.java"])[0] and not S.gate_compile(["src/B.java"])[0]


def test_stage_fallida_es_excepcion_con_etapa(run):
    @S.stage("9-falsa")
    def falsa():
        return False, "motivo"
    with pytest.raises(S.StageFailed) as e:
        falsa()
    assert e.value.stage == "9-falsa" and e.value.note == "motivo"
    assert S.state["stages"][-1]["ok"] is False


def test_build_feature_exige_oraculo(tmp_path):
    with pytest.raises(ValueError, match="accept"):
        S.build_feature("x", "t", str(tmp_path))


def test_build_feature_serializa_corridas_paralelas(tmp_path, monkeypatch):
    """Dos jobs del server en el mismo proceso compartian los globals: uno leia el worktree del otro."""
    import threading
    import time as _time
    activos, pico = [0], [0]

    def run(_stage):
        activos[0] += 1
        pico[0] = max(pico[0], activos[0])
        _time.sleep(0.05)
        activos[0] -= 1
        return {}
    monkeypatch.setattr(S, "configure", lambda *a, **k: None)
    monkeypatch.setattr(S, "run", run)
    hilos = [threading.Thread(target=S.build_feature, args=(f"f{i}", "t", str(tmp_path)),
                              kwargs={"accept_cmd": "x", "files": ["a.py"]}) for i in range(3)]
    for h in hilos:
        h.start()
    for h in hilos:
        h.join()
    assert pico[0] == 1


def test_reviewer_configurable_por_repo(tmp_path, monkeypatch):
    """Ticket 11: el agente que revisa y corrige sale de `reviewer_cmd` del repo; sin esa clave manda la seam."""
    import mmorch.claude_exec as CE
    (tmp_path / "sdlc.toml").write_text(
        'files = ["a.py"]\nreviewer_cmd = "python -c \\"import sys; print(\'modo\', __import__(\'os\').environ[\'MMORCH_MODE\'],'
        ' sys.stdin.read()[:12])\\""\n', encoding="utf-8")
    t = types.SimpleNamespace(name="rev", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["a.py"], "suite": []}, wt=tmp_path)
    r = S._revisor("REVISA ESTO y nada mas", mode="edit")
    assert r["ok"] and "modo edit" in r["result"] and "REVISA ESTO" in r["result"]
    llamado = []
    monkeypatch.setattr(CE, "get_executor", lambda: types.SimpleNamespace(
        run=lambda *a, **k: llamado.append(k) or CE.ExecResult(True, "seam", 0, 1)))
    S.CFG.pop("reviewer_cmd")
    assert S._revisor("x")["result"] == "seam" and llamado[0]["mode"] == "edit"


def _repo_git(wt):
    (wt / "pkg").mkdir(parents=True)
    (wt / "pkg" / "viejo.py").write_text("def f(x):\n    return x\n", encoding="utf-8")
    for c in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "b"]):
        subprocess.run(["git", *c], cwd=wt, check=True)


def test_codigo_muerto_en_archivo_nuevo_bloquea_bajo_el_umbral(tmp_path):
    """macro_leadlag llego con ~180 lineas que ningun test ejecutaba; ningun gate miraba archivos nuevos."""
    wt = tmp_path / "wt"
    _repo_git(wt)
    acc = "def test_R1():\n    from pkg.nuevo import doble\n    assert doble(2) == 4\n"
    (wt / "test_acc.py").write_text(acc, encoding="utf-8")
    muerto = "".join(f"    if formato == {i}:\n        return {i}\n" for i in range(12))
    (wt / "pkg" / "nuevo.py").write_text(f"def doble(x):\n    return 2 * x\n\n\ndef formatos(formato):\n{muerto}", encoding="utf-8")
    t = types.SimpleNamespace(name="m", task="t", accept_files={"test_acc.py": acc})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["pkg/nuevo.py"], "suite": []}, wt=wt)
    S.PY = sys.executable
    S.state["plan_files"] = ["pkg/nuevo.py", "pkg/viejo.py"]
    ok, nota = S.gate_codigo_muerto()
    assert not ok and "pkg/nuevo.py" in nota and "viejo" not in nota  # el archivo de HEAD no se mide
    (wt / "pkg" / "nuevo.py").write_text("def doble(x):\n    return 2 * x\n", encoding="utf-8")
    ok, nota = S.gate_codigo_muerto()
    assert ok and S.state["cobertura_nueva"] == {"pkg/nuevo.py": 1.0}, nota


def test_test_de_revision_se_llama_como_la_feature(tmp_path):
    """Un nombre fijo (tests/test_review_sdlc.py) hacia chocar la revision de dos features al mergear."""
    casos = {"tests/test_sdlc_macro_leadlag.py": "tests/test_review_macro_leadlag.py",
             "app/src/sdlc/export_mastery.test.ts": "tests/test_review_export_mastery.py",
             "backend/src/test/java/Sdlc_login_Test.java": "tests/test_review_sdlc_login.py"}
    for acc, esperado in casos.items():
        t = types.SimpleNamespace(name="298c10307f", task="t", accept_files={acc: ""})
        S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["a.py"], "suite": []}, wt=tmp_path)
        assert S.REVIEW_REL == esperado
    S.configure(types.SimpleNamespace(name="298c10307f", task="t", accept_files={}), contract=[],
                feat={"repo": str(tmp_path), "files": ["a.py"], "suite": []}, wt=tmp_path)
    assert S.REVIEW_REL == "tests/test_review_298c10307f.py"


def test_mutacion_solo_en_las_lineas_que_cambio_la_feature(tmp_path):
    """El score medía el archivo entero (0.25 en canal.py): los mutantes caian en codigo viejo que la feature no toca."""
    from mmorch.checkers import _mutants
    wt = tmp_path / "wt"
    _repo_git(wt)
    t = types.SimpleNamespace(name="m", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["pkg/viejo.py"], "suite": []}, wt=wt)
    codigo = "def f(x):\n    return x\n\n\ndef g(x):\n    return x + 1 if x > 0 else 0\n"
    (wt / "pkg" / "viejo.py").write_text(codigo, encoding="utf-8")
    assert S._lineas_cambiadas("pkg/viejo.py") == {3, 4, 5, 6}
    assert S._lineas_cambiadas("pkg/no_existe.py") is None  # archivo nuevo: todo cuenta
    solo_g = _mutants(codigo, max_n=50, lineas={6})
    assert solo_g and all(m.splitlines()[:2] == ["def f(x):", "    return x"] for m in solo_g)
    assert _mutants(codigo, max_n=50, lineas={2}) == []  # f no tiene nada mutable
    assert S._mutantes_texto("a = 1 == 2\nb = 3 == 4\n", lineas={2}) == ["a = 1 == 2\nb = 3 != 4\n"]


def test_lint_cuenta_hallazgos_nuevos_por_archivo_contra_la_base(tmp_path, monkeypatch):
    """Ticket 1 del mapa portfolio-circuito: la deuda vieja no bloquea; un hallazgo NUEVO si. Archivo nuevo parte de 0."""
    import subprocess
    import types as _t
    wt = tmp_path / "wt"
    (wt / "pkg").mkdir(parents=True)
    (wt / "pkg" / "viejo.py").write_text("l = 1\n", encoding="utf-8")  # E741 preexistente
    for c in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "b"]):
        subprocess.run(["git", *c], cwd=wt, check=True)
    t = _t.SimpleNamespace(name="l", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["pkg/viejo.py"], "suite": []}, wt=wt)
    S.state["plan_files"] = ["pkg/viejo.py"]
    (wt / "pkg" / "viejo.py").write_text("l = 1\nx = 2\n", encoding="utf-8")  # cambio sin hallazgos nuevos
    ok, nota = S.gate_lint()
    assert ok, nota
    assert S.state["lint_new"] == {"pkg/viejo.py": [1, 1]}
    (wt / "pkg" / "viejo.py").write_text("l = 1\nI = 2\n", encoding="utf-8")  # E741 NUEVO
    assert not S.gate_lint()[0]
    S.state["plan_files"] = ["pkg/nuevo.py"]
    (wt / "pkg" / "nuevo.py").write_text("O = 3\n", encoding="utf-8")  # archivo nuevo con hallazgo: base 0
    ok, nota = S.gate_lint()
    assert not ok and "nuevo.py" in nota
    assert (wt / "pkg" / "nuevo.py").exists()  # el baseline lo restaura
