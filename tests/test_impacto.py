"""mmorch.impacto: informe de literales compartidos (mapa .scratch/mapa-de-impacto, ticket 05)."""
import json

import pytest

import mmorch.impacto as I


def _repo(tmp_path, files):
    (tmp_path / ".git").mkdir(parents=True)
    for rel, body in files.items():
        p = tmp_path / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body, encoding="utf-8")
    return tmp_path


WRITER = 'import sqlite3\n\ndef save(c, t, d):\n    c.execute("INSERT INTO orders (total, discount) VALUES (?, ?)", (t, d))\n'
READER = 'def revenue(c):\n    return c.execute("SELECT SUM(total - discount) FROM orders").fetchone()[0]\n'


def test_lista_al_lector_acoplado_y_los_tests(tmp_path):
    root = _repo(tmp_path, {"orders.py": WRITER, "reports/nightly.py": READER, "util.py": "X = 1\n",
                            "tests/test_orders.py": "import orders\n"})
    r = I.report(root, root / "orders.py")
    assert "reports/nightly.py" in r and "`discount`" in r
    assert "util.py" not in r
    assert "tests/test_orders.py" not in r   # sin linea de tests (aceptacion del ticket 05)


def test_prosa_no_cuenta_como_dato(tmp_path):
    root = _repo(tmp_path, {"a.py": 'MSG = "no se pudo guardar el pedido"\n',
                            "b.py": 'ERR = "fallo al guardar el pedido del cliente"\n'})
    assert I.report(root, root / "a.py") == ""


def test_orden_por_especificidad_y_literal_comun_resumido(tmp_path):
    files = {"w.py": 'A = "clave_rara"\nB = "clave_comun"\n', "r.py": 'X = "clave_rara"\n'}
    files |= {f"m{i}.py": 'Y = "clave_comun"\n' for i in range(I.COMMON + 1)}
    root = _repo(tmp_path, files)
    lines = I.report(root, root / "w.py").splitlines()
    assert "`clave_rara`" in lines[1] and "r.py" in lines[1]
    assert "`clave_comun`" in lines[2] and "literal comun" in lines[2]


def test_tope_de_lineas(tmp_path):
    keys = [f"k{i:02d}" for i in range(I.MAX_LINES + 5)]
    root = _repo(tmp_path, {"w.py": "\n".join(f'V{i} = "{k}"' for i, k in enumerate(keys)) + "\n",
                            "r.py": "\n".join(f'W{i} = "{k}"' for i, k in enumerate(keys)) + "\n"})
    body = [ln for ln in I.report(root, root / "w.py").splitlines() if ln.startswith("- ")]
    assert len(body) == I.MAX_LINES and "literales compartidos mas" in body[-1]


def test_hook_una_vez_por_archivo_y_solo_py(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path / "repo", {"orders.py": WRITER, "reports/nightly.py": READER})
    ev = {"session_id": "s1", "tool_input": {"file_path": str(root / "orders.py")}}
    out = json.loads(I.hook(json.dumps(ev)))
    assert "reports/nightly.py" in out["hookSpecificOutput"]["additionalContext"]
    assert "permissionDecision" not in out["hookSpecificOutput"]
    assert I.hook(json.dumps(ev)) == ""                      # segunda vez en la sesion: nada
    assert I.hook(json.dumps({"session_id": "s1", "tool_input": {"file_path": str(root / "x.md")}})) == ""
    assert (tmp_path / "home" / "logs" / "impacto.jsonl").exists()


def test_hook_sin_repo_no_informa_ni_recorre(tmp_path, monkeypatch):
    if any((d / ".git").exists() for d in tmp_path.parents):
        pytest.skip("tmp_path cae dentro de un repo git")
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    for rel, body in {"orders.py": WRITER, "reports/nightly.py": READER}.items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(body, encoding="utf-8")
    monkeypatch.setattr(I, "_source_files", lambda *a: pytest.fail("recorrio sin repo"))
    ev = {"session_id": "s-sin-repo", "tool_input": {"file_path": str(tmp_path / "orders.py")}}
    assert I.hook(json.dumps(ev)) == ""


def test_hook_modo_cursor(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path / "repo", {"orders.py": WRITER, "reports/nightly.py": READER})
    ev = {"conversation_id": "c1", "tool_name": "Write", "tool_input": {"path": str(root / "orders.py")}}
    out = json.loads(I.hook(json.dumps(ev), cursor=True))
    assert "reports/nightly.py" in out["additional_context"]
    assert I.hook(json.dumps(ev), cursor=True) == ""


def test_typescript_lista_al_lector_y_saltea_imports(tmp_path):
    pytest.importorskip("tree_sitter_typescript")
    root = _repo(tmp_path, {
        "exporter.ts": 'import { writeFileSync } from "node:fs";\nexport const H = "date,amount_cents";\n',
        "reconcile.ts": 'import { readFileSync } from "node:fs";\nconst c = h.indexOf("amount_cents");\n',
        "fmt.ts": 'export const money = (x: number) => x.toFixed(2);\n',
        "exporter.test.ts": 'const x = "amount_cents";\n'})
    r = I.report(root, root / "exporter.ts")
    assert "reconcile.ts" in r and "`amount_cents`" in r
    assert "`node`" not in r and "exporter.test.ts" not in r


def test_java_lista_al_lector_y_saltea_tests(tmp_path):
    pytest.importorskip("tree_sitter_java")
    root = _repo(tmp_path, {
        "Exporter.java": 'import java.nio.file.*;\nclass Exporter { String h = "date,amount_cents"; }\n',
        "Reconcile.java": 'class Reconcile { int c = h.indexOf("amount_cents"); }\n',
        "Fmt.java": 'class Fmt { String f = "$%.2f"; }\n',
        "ExporterTest.java": 'class ExporterTest { String x = "amount_cents"; }\n'})
    r = I.report(root, root / "Exporter.java")
    assert "Reconcile.java" in r and "`amount_cents`" in r
    assert "ExporterTest.java" not in r


def test_extension_sin_soporte_no_informa(tmp_path):
    root = _repo(tmp_path, {"a.rb": 'k = "clave"\n', "b.rb": 'k = "clave"\n'})
    assert I.report(root, root / "a.rb") == ""


def test_script_del_hook_no_carga_el_init_del_paquete():
    """mmorch/__init__ importa providers y openai (~8 s); el hook corta a los 15 s."""
    import subprocess
    import sys
    from pathlib import Path
    script = Path(__file__).resolve().parents[1] / "scripts" / "impacto_hook.py"
    r = subprocess.run([sys.executable, "-X", "importtime", str(script), "hook"], input="{}",
                       capture_output=True, text=True, timeout=120)
    assert r.returncode == 0 and "mmorch.impacto" in r.stderr
    assert "openai" not in r.stderr and "mmorch.providers" not in r.stderr


def test_hook_falla_abierto(capsys, monkeypatch):
    monkeypatch.setattr(I.sys, "stdin", type("S", (), {"read": lambda self: "{no es json"})())
    assert I.main(["hook"]) == 0
    assert capsys.readouterr().out == ""
