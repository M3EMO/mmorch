"""Gates del pipeline en lenguajes sin AST en stdlib (Rust, JS, ...): mutantes por texto, cabecera de comentario como
docstring, clones por lineas. Cero API, cero compilador real: accept_cmd/compile_cmd son scripts Python."""
import sys
import types

import mmorch.sdlc as S

JS = "// modulo a\n// segunda linea\nconst x = require('y');\nfunction f(a, b) {\n  if (a == b && a > 0) { return a + b; }\n  return false;\n}\n"
RS = "//! crate doc\n//! linea 2\nuse std::fmt;\npub fn f(a: i32) -> bool {\n    a >= 1 && true\n}\n"


def _wt(tmp_path, monkeypatch, name, code, accept_cmd, compile_cmd=None):
    (tmp_path / "src").mkdir(parents=True, exist_ok=True)
    (tmp_path / "src" / name).write_text(code, encoding="utf-8")
    t = types.SimpleNamespace(name="l", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": [f"src/{name}"], "suite": []}, wt=tmp_path, accept_cmd=accept_cmd)
    S.CFG["ext"] = ["js", "rs"]
    if compile_cmd:
        S.CFG["compile_cmd"] = compile_cmd
    S.state["plan_files"] = [f"src/{name}"]


def test_mutantes_texto_un_swap_por_mutante_y_sin_comentarios():
    ms = S._mutantes_texto(JS, max_n=8)
    assert ms and all(m != JS for m in ms)
    assert any("a != b" in m for m in ms) and any("a || a > 0" in m or "&& a > 0" not in m for m in ms)
    assert all(m.startswith("// modulo a\n// segunda linea\n") for m in ms)  # los comentarios no mutan
    assert all("require('y')" in m for m in ms)  # los imports no mutan


def test_mutacion_en_js_con_accept_cmd_y_mortinatos(tmp_path, monkeypatch):
    # el "test" del repo: pasa solo si el archivo conserva `a == b`; el "compilador": falla si aparece `a || a`
    acc = "python -c \"import sys; sys.exit(0 if 'a == b' in open('src/a.js', encoding='utf-8').read() else 1)\""
    comp = "python -c \"import sys; sys.exit(1 if 'a || a' in open('src/a.js', encoding='utf-8').read() else 0)\""
    _wt(tmp_path, monkeypatch, "a.js", JS, acc, comp)
    ok, nota = S.gate_mutacion()
    assert ok and "observa" in nota
    score = S.state["mutation_score"]
    assert 0 < score < 1  # mata el swap de ==, no mata el resto; el mutante `||` es mortinato y no cuenta
    assert (tmp_path / "src" / "a.js").read_text(encoding="utf-8") == JS  # restaurado


def test_docstring_generico_es_la_cabecera_de_comentario(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch, "lib.rs", RS, "python -c \"pass\"")
    antes = S._docstrings(["src/lib.rs"])
    assert antes["src/lib.rs"] == "//! crate doc\n//! linea 2"
    (tmp_path / "src" / "lib.rs").write_text(RS.replace("//! crate doc\n//! linea 2\n", ""), encoding="utf-8")
    ok, nota = S.gate_docstring(antes)
    assert not ok and "lib.rs" in nota


def test_clones_no_dependen_del_lenguaje(tmp_path, monkeypatch):
    _wt(tmp_path, monkeypatch, "a.js", JS, "python -c \"pass\"")
    ok, _ = S.gate_clones({"src/a.js": JS, "src/b.js": JS.replace("modulo a", "modulo b")})
    assert not ok


def test_suite_cmd_compara_verde_rojo_contra_el_baseline(tmp_path, monkeypatch):
    """Repo no pytest (Estudio con vitest, ChatBot con maven): la suite total corre `suite_cmd` y compara con HEAD."""
    import subprocess
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    wt = tmp_path / "wt"
    (wt / "src").mkdir(parents=True)
    (wt / "src" / "a.js").write_text("const ok = true;\n", encoding="utf-8")
    (wt / "suite.py").write_text("import sys; sys.exit(0 if 'true' in open('src/a.js').read() else 1)", encoding="utf-8")
    for c in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "b"]):
        subprocess.run(["git", *c], cwd=wt, check=True)
    t = types.SimpleNamespace(name="s", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["src/a.js"], "suite": []}, wt=wt)
    S.CFG["suite_cmd"] = "python suite.py"
    S.state.update(plan_files=["src/a.js"], base_sha="HEAD")
    (wt / "src" / "a.js").write_text("const ok = false;\n", encoding="utf-8")  # el cambio del plan rompe la suite
    ok, nota = S.gate_suite_total()
    assert not ok and "ahora rojo" in nota
    assert (wt / "src" / "a.js").read_text(encoding="utf-8") == "const ok = false;\n"  # el baseline restaura el cambio
    (wt / "src" / "a.js").write_text("const ok = true;\n", encoding="utf-8")
    assert S.gate_suite_total()[0]


def test_baseline_con_archivo_nuevo_del_plan(tmp_path, monkeypatch):
    """macro-leadlag y export-mastery CREAN un archivo: el baseline hacia `git checkout HEAD -- nuevo` y explotaba
    (pathspec no conocido por git), el job terminaba en error sin detalle. En la base el archivo nuevo no existe."""
    import subprocess
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    wt = tmp_path / "wt"
    (wt / "src").mkdir(parents=True)
    (wt / "suite.py").write_text("import os, sys; sys.exit(0 if not os.path.exists('src/nuevo.js') or 'ok' in open('src/nuevo.js').read() else 1)",
                                 encoding="utf-8")
    for c in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "b"]):
        subprocess.run(["git", *c], cwd=wt, check=True)
    t = types.SimpleNamespace(name="n", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["src/nuevo.js"], "suite": []}, wt=wt)
    S.CFG["suite_cmd"] = "python suite.py"
    S.state.update(plan_files=["src/nuevo.js"], base_sha="HEAD")
    (wt / "src" / "nuevo.js").write_text("const x = 'ok';\n", encoding="utf-8")  # archivo NUEVO, sin trackear
    ok, nota = S.gate_suite_total()
    assert ok, nota
    assert (wt / "src" / "nuevo.js").read_text(encoding="utf-8") == "const x = 'ok';\n"  # el baseline lo restaura


def test_baseline_sin_la_aceptacion_ve_regresiones_en_lenguajes_compilados(tmp_path, monkeypatch):
    """ChatBot (maven): el test de aceptacion no compila en la base y tumba la suite entera; el baseline salia rojo
    siempre y el gate de regresion solo observaba. La base se mide sin los tests de aceptacion."""
    import subprocess
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    wt = tmp_path / "wt"
    (wt / "src").mkdir(parents=True)
    (wt / "src" / "viejo.js").write_text("const ok = true;\n", encoding="utf-8")
    # "compilador": la suite cae si hay un test de aceptacion sin su implementacion, o si viejo.js se rompe
    (wt / "suite.py").write_text("import os, sys\nacc = os.path.exists('src/acc.test.js') and not os.path.exists('src/nuevo.js')\n"
                                 "sys.exit(1 if acc or 'true' not in open('src/viejo.js').read() else 0)\n", encoding="utf-8")
    (wt / "src" / "acc.test.js").write_bytes(b"test('R1')\r\n")
    for c in (["init", "-q"], ["add", "-A"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "b"]):
        subprocess.run(["git", *c], cwd=wt, check=True)
    t = types.SimpleNamespace(name="c", task="t", accept_files={"src/acc.test.js": "test('R1')\n"})
    S.configure(t, contract=[], feat={"repo": str(wt), "files": ["src/nuevo.js", "src/viejo.js"], "suite": []}, wt=wt)
    S.CFG["suite_cmd"] = "python suite.py"
    S.state.update(plan_files=["src/nuevo.js", "src/viejo.js"], base_sha="HEAD")
    (wt / "src" / "nuevo.js").write_text("const x = 1;\n", encoding="utf-8")
    (wt / "src" / "viejo.js").write_text("const ok = false;\n", encoding="utf-8")  # la feature rompe codigo viejo
    ok, nota = S.gate_suite_total()
    assert not ok and "ahora rojo" in nota and S.state["suite_total"]["baseline_verde"], nota
    assert (wt / "src" / "acc.test.js").read_bytes() == b"test('R1')\r\n"  # la aceptacion vuelve intacta


def test_aceptacion_en_otro_lenguaje_tambien_corre_el_test_de_revision(tmp_path, monkeypatch):
    """ChatBot: Claude bloqueo con un test pytest que fallaba, pero `mvn test` no lo corria y el PR siguio."""
    t = types.SimpleNamespace(name="r", task="t", accept_files={"src/Sdlc_x_Test.java": ""})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["src/X.java"], "suite": []}, wt=tmp_path,
                accept_cmd="python -c \"pass\"")
    S.PY = sys.executable
    assert S.accept()[0]  # sin test de revision: manda el comando del repo
    (tmp_path / S.REVIEW_REL).parent.mkdir(parents=True, exist_ok=True)
    (tmp_path / S.REVIEW_REL).write_text("def test_R2_colision():\n    assert 1 == 2\n", encoding="utf-8")
    ok, log = S.accept()
    assert not ok and "1 failed" in log
