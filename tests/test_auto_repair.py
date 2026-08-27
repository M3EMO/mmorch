"""Tests de auto_repair (fakes, sin engine ni API)."""

import json

from mmorch.auto_repair import findings_from_record, pick_finding, _sig


def test_findings_extraction_all_shapes():
    rec = {
        "ts": 1,
        "evolve_error": "FileNotFoundError: gh",
        "hardening": {"error": "F821 hashlib"},
        "idea_loop": {"errors": ["adjudicate: boom"], "steps": {}},
        "project_health": {"errors": ["Portfolio: timeout"], "ok": []},
        "autoresearch": {"baseline": 0.8},  # sin error -> no aporta
    }
    f = findings_from_record(rec)
    detalles = {x["detail"] for x in f}
    assert "FileNotFoundError: gh" in detalles
    assert "F821 hashlib" in detalles
    assert "adjudicate: boom" in detalles
    # project_health = errores de OTROS repos: excluidos (irreparables aca)
    assert "Portfolio: timeout" not in detalles
    assert len(f) == 3


def test_pick_respects_retry_window():
    f1 = {"source": "evolve_error", "detail": "X" * 100}
    f2 = {"source": "hardening", "detail": "Y"}
    state = {_sig(f1): {"retry_after": "2026-08-25"}}
    picked = pick_finding([f1, f2], state, today="2026-08-19")
    assert picked["source"] == "hardening"
    # pasada la ventana vuelve a ser elegible
    assert pick_finding([f1, f2], state, today="2026-08-26")["source"] == "evolve_error"


def test_pick_none_when_all_recent():
    f1 = {"source": "a", "detail": "d"}
    assert pick_finding([f1], {_sig(f1): {"retry_after": "2099-01-01"}},
                        today="2026-08-19") is None


def test_sig_stable_despite_variable_tail():
    a = {"source": "s", "detail": "F821 hashlib en " + "x" * 100 + "path1"}
    b = {"source": "s", "detail": "F821 hashlib en " + "x" * 100 + "path2"}
    assert _sig(a) == _sig(b)  # el prefijo manda; los paths del final no


def test_repair_skips_without_record(tmp_path):
    from mmorch.auto_repair import repair
    (tmp_path / "logs").mkdir()
    r = repair(str(tmp_path), today="2026-08-19")
    assert r == {"skipped": "sin record nocturno"}


def test_repair_skips_paused(tmp_path):
    from mmorch.auto_repair import repair
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "loop_paused").touch()
    (logs / "nightly.jsonl").write_text(json.dumps({"x_error": "boom"}) + "\n",
                                        encoding="utf-8")
    assert repair(str(tmp_path), today="2026-08-19") == {"skipped": "paused"}


def test_repair_usa_rec_en_memoria_sin_leer_nightly_jsonl(tmp_path):
    """Pasar rec= evita el lag de 1 noche: repara lo de ESTA corrida, no lo
    de ayer (nightly.py llama repair(rec=rec) despues de mover el llamado
    al final del script).

    git init EXPLICITO: la version sin init pasaba solo de casualidad — el
    tmp de pytest caia DENTRO del repo de orchestration y `git -C` encontraba
    al padre; en un sandbox de evolve (basetemp fuera de todo repo) reventaba
    con 'not a git repo' y mato los 6 sandboxes del estreno. Un test que
    depende de DONDE corre no es un test."""
    import subprocess as _sp
    from mmorch.auto_repair import repair
    logs = tmp_path / "logs"
    logs.mkdir()
    for cmd in (["git", "init", "-b", "main"],
                ["git", "config", "user.email", "t@t"],
                ["git", "config", "user.name", "t"]):
        _sp.run(cmd, cwd=tmp_path, capture_output=True)
    (tmp_path / "x.py").write_text("x = 1\n", encoding="utf-8")
    _sp.run(["git", "add", "-A"], cwd=tmp_path, capture_output=True)
    _sp.run(["git", "commit", "-m", "base", "--no-verify"], cwd=tmp_path,
            capture_output=True)
    # sin record en disco -> con rec= no deberia importar
    rec_hoy = {"ts": 1, "algo": {"error": "boom de HOY"}}
    r = repair(str(tmp_path), today="2026-08-19", rec=rec_hoy,
              build_fn=lambda t, w, g: {"status": "built"})
    assert r.get("status") != "sin record nocturno"
    assert "no deberia" not in str(r)  # sanity: no exploto por falta de archivo


def test_repair_persiste_estado_despues_del_automerge(tmp_path, monkeypatch):
    """05 #6: el estado se escribe DESPUES del automerge y lo incluye — un crash
    entre medio ya no deja repair_state sin el resultado real del merge."""
    import subprocess as _sp
    from mmorch.auto_repair import repair
    logs = tmp_path / "logs"
    logs.mkdir()
    for cmd in (["git", "init", "-b", "main"],
                ["git", "config", "user.email", "t@t"],
                ["git", "config", "user.name", "t"]):
        _sp.run(cmd, cwd=tmp_path, capture_output=True)
    (tmp_path / "x.py").write_text("x = 1\n", encoding="utf-8")
    _sp.run(["git", "add", "-A"], cwd=tmp_path, capture_output=True)
    _sp.run(["git", "commit", "-m", "base", "--no-verify"], cwd=tmp_path,
            capture_output=True)

    import mmorch.automerge as am
    state_path = logs / "repair_state.json"
    visto = {}

    def fake_automerge(repo, branch, *, base, source=""):
        # el orden es el contrato: al momento del automerge NADA persistido aun
        visto["estado_ya_persistido"] = state_path.exists()
        return {"merged": True, "zone": "green", "veredicto": "merged"}

    monkeypatch.setattr(am, "try_automerge", fake_automerge)
    r = repair(str(tmp_path), today="2026-08-19",
               rec={"ts": 1, "algo": {"error": "boom"}},
               build_fn=lambda t, w, g: {"status": "built"})
    assert r["automerge"]["merged"] is True
    assert visto["estado_ya_persistido"] is False
    state = json.loads(state_path.read_text(encoding="utf-8"))
    entry = next(iter(state.values()))
    assert entry["automerge"]["veredicto"] == "merged"
