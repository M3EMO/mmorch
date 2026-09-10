"""synth_store: put/get/run y la regla 'mas evidencia reemplaza, menos no'. $0, sin API."""
import mmorch.synth_store as ss


def test_roundtrip_and_evidence_rule(tmp_path, monkeypatch):
    monkeypatch.setattr(ss, "STORE_PATH", tmp_path / "s.json")
    src = "def solve(params):\n    return params['n'] * 2\n"
    assert ss.get("doble") is None
    assert ss.put("doble", src, "m1", {"n_promote": 4, "edge": True, "n_test": 7, "n_test_ok": 7})
    assert ss.get("doble") == src
    assert ss.run("doble", {"n": 21}) == 42
    assert ss.run("inexistente", {"n": 1}) is None
    # menos evidencia NO reemplaza
    assert not ss.put("doble", "def solve(p):\n    return 0\n", "m2",
                      {"n_promote": 4, "edge": True, "n_test": 3, "n_test_ok": 3})
    assert ss.run("doble", {"n": 1}) == 2
    # mas evidencia SI reemplaza
    assert ss.put("doble", "def solve(p):\n    return p['n'] + p['n']\n", "m3",
                  {"n_promote": 4, "edge": True, "n_test": 17, "n_test_ok": 17})
    assert ss.entry("doble")["model"] == "m3"
    assert ss.stats()["kinds"] == 1


def test_broken_checker_returns_none(tmp_path, monkeypatch):
    monkeypatch.setattr(ss, "STORE_PATH", tmp_path / "s.json")
    ss.put("roto", "def solve(p):\n    return 1 / 0\n", "m", {"n_promote": 1, "n_test_ok": 0})
    assert ss.run("roto", {}) is None
