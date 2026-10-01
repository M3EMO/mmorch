"""Chequeo de costo despues de escribir (ticket 15): trampas, falsas alarmas y hook post."""
import json

from mmorch import impacto as I

CUSTOMERS = '''import json
import os
import sqlite3


def _path():
    return os.path.join(os.environ.get("APP_DATA", "."), "customers.jsonl")


def get_customer(cid):
    with open(_path(), encoding="utf-8") as fh:
        for line in fh:
            if json.loads(line)["id"] == cid:
                return json.loads(line)


def read_note(path):
    return open(path, encoding="utf-8").read()


def log(x):
    with open(_path(), "a", encoding="utf-8") as fh:
        fh.write(x)


def find(email):
    con = sqlite3.connect("users.db")
    return [r for r in con.execute("SELECT email FROM users") if r[0] == email]


def find_one(email):
    con = sqlite3.connect("users.db")
    return con.execute("SELECT email FROM users WHERE email = ?", (email,)).fetchone()


def get_name(cid):
    return get_customer(cid)["name"]
'''


def _repo(tmp_path, report_src):
    root = tmp_path / "repo"
    (root / ".git").mkdir(parents=True)
    (root / "customers.py").write_text(CUSTOMERS, encoding="utf-8")
    (root / "report.py").write_text(report_src, encoding="utf-8")
    return root


def _check(tmp_path, monkeypatch, src):
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir(exist_ok=True)
    root = _repo(tmp_path, src)
    return I.cost_report(root, root / "report.py")


def test_trampa_en_for_comprehension_y_key(tmp_path, monkeypatch):
    src = ("from customers import get_customer, get_name\n\n\ndef rows(orders):\n"
           "    out = [get_customer(o) for o in orders]\n"
           "    for o in orders:\n        out.append(get_name(o))\n"
           "    return sorted(orders, key=get_customer)\n")
    r = _check(tmp_path, monkeypatch, src)
    assert "- linea 5: `get_customer` (customers.py:10) lee `customers.jsonl` completo en cada llamada" in r
    assert "- linea 7: `get_name` (customers.py:36) llama a `get_customer`, que lee `customers.jsonl`" in r


def test_sql_sin_where_si_con_where_no(tmp_path, monkeypatch):
    src = ("import customers\n\n\ndef dedupe(emails):\n"
           "    a = [customers.find(e) for e in emails]\n"
           "    b = [customers.find_one(e) for e in emails]\n    return a, b\n")
    r = _check(tmp_path, monkeypatch, src)
    assert "`find` (customers.py:26) lee la tabla `users` completa" in r and "find_one" not in r


def test_falsas_alarmas_descartadas(tmp_path, monkeypatch):
    src = ("from customers import get_customer, log, read_note\n\n\ndef go(paths, orders):\n"
           "    notes = [read_note(p) for p in paths]\n"      # lee lo que le pasan: archivo distinto
           "    for o in orders:\n        log(o)\n"               # escribe, no recorre
           "    return [o for o in get_customer(1)]\n")          # primer iterable: corre una vez
    assert _check(tmp_path, monkeypatch, src) == ""


def test_metodo_homonimo_no_hereda_la_marca(tmp_path, monkeypatch):
    # `d.items()` es el metodo de un dict, no la funcion `items` del modulo que recorre q.json (falso positivo
    # visto el 2026-10-01 en orch-spike/scripts/cursor_roles/roles.py)
    src = ("import json\n\n\ndef items():\n    return json.loads(open('q.json').read())\n\n\n"
           "def pares(d):\n    return list(d.items())\n\n\n"
           "def todo(ds):\n    return [pares(d) for d in ds]\n")
    assert _check(tmp_path, monkeypatch, src) == ""


def test_hook_post_avisa_solo_cuando_cambia(tmp_path, monkeypatch):
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path, "from customers import get_customer\n\nX = [get_customer(i) for i in range(3)]\n")
    ev = json.dumps({"session_id": "s1", "tool_input": {"file_path": str(root / "report.py")}})
    out = json.loads(I.post(ev))["hookSpecificOutput"]
    assert out["hookEventName"] == "PostToolUse" and "get_customer" in out["additionalContext"]
    assert I.post(ev) == ""                                   # mismo aviso: silencio
    (root / "report.py").write_text("# linea nueva\n\nfrom customers import get_customer\n\nX = [get_customer(i) for i in range(3)]\n",
                                    encoding="utf-8")
    assert I.post(ev) == ""                                   # solo se corrieron las lineas: silencio
    (root / "report.py").write_text("X = 1\n", encoding="utf-8")
    assert I.post(ev) == ""                                   # arreglado: nada que avisar
    (root / "report.py").write_text("from customers import get_customer\n\nY = [get_customer(i) for i in range(2)]\n",
                                    encoding="utf-8")
    assert "get_customer" in I.post(ev)                       # vuelve la trampa: avisa de nuevo


def test_modo_cursor_suma_el_chequeo(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path, "from customers import get_customer\n\nX = [get_customer(i) for i in range(3)]\n")
    ev = json.dumps({"conversation_id": "c1", "tool_input": {"path": str(root / "report.py")}})
    assert "COSTO (automatico)" in json.loads(I.hook(ev, cursor=True))["additional_context"]
