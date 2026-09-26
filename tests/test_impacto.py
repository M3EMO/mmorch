"""mmorch.impacto: informe de literales compartidos (mapa .scratch/mapa-de-impacto, ticket 05)."""
import json

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


def test_hook_modo_cursor(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path / "repo", {"orders.py": WRITER, "reports/nightly.py": READER})
    ev = {"conversation_id": "c1", "tool_name": "Write", "tool_input": {"path": str(root / "orders.py")}}
    out = json.loads(I.hook(json.dumps(ev), cursor=True))
    assert "reports/nightly.py" in out["additional_context"]
    assert I.hook(json.dumps(ev), cursor=True) == ""


def test_hook_falla_abierto(capsys, monkeypatch):
    monkeypatch.setattr(I.sys, "stdin", type("S", (), {"read": lambda self: "{no es json"})())
    assert I.main(["hook"]) == 0
    assert capsys.readouterr().out == ""
