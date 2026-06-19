import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def _sess(tmp_path, events):
    p = tmp_path / "s.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in events), encoding="utf-8")
    return p


def _calib_session(tmp_path):
    # 1 segmento: 1 tool-call (observed=clear) + tests passed (outcome positivo).
    return _sess(tmp_path, [
        {"type": "user", "message": {"role": "user", "content": "lee el archivo"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "3 passed", "is_error": False}]}},
    ])


def test_records_calibration_with_predicted_domain(tmp_path):
    rec = []
    led = tmp_path / "ledger.txt"
    rep = S.ingest_session(_calib_session(tmp_path),
                           recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "complex"})(),
                           ledger=led)
    assert rep.recorded == 1 and rep.segments == 1
    k = rec[0]
    assert k["arm"] == "cynefin:complex" and k["reward"] == 0.0
    assert k["source"] == "claude_session"


def test_matching_domain_rewards_one(tmp_path):
    rec = []
    rep = S.ingest_session(_calib_session(tmp_path),
                           recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "clear"})(),
                           ledger=tmp_path / "l.txt")
    assert rec[0]["reward"] == 1.0


def test_no_signal_segment_is_skipped(tmp_path):
    rec = []
    p = _sess(tmp_path, [
        {"type": "user", "message": {"role": "user", "content": "contame algo"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "una historia"}]}}])
    rep = S.ingest_session(p, recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "clear"})(),
                           ledger=tmp_path / "l.txt")
    assert rep.recorded == 0 and rep.skipped_no_signal == 1 and rec == []


def test_idempotent_second_ingest_skips(tmp_path):
    rec = []
    led = tmp_path / "l.txt"
    args = dict(recorder=lambda **k: rec.append(k),
                classifier=lambda req, **k: type("R", (), {"domain": "clear"})(), ledger=led)
    p = _calib_session(tmp_path)
    S.ingest_session(p, **args)
    rep2 = S.ingest_session(p, **args)
    assert rep2.already_ingested and rep2.recorded == 0 and len(rec) == 1


def test_grown_session_does_not_double_count(tmp_path):
    # backport del modelo incremental: una sesion que CRECE solo registra los segmentos
    # nuevos; no re-registra (no doble-cuenta) los outcomes de calibracion previos.
    rec = []
    led = tmp_path / "l.txt"
    args = dict(recorder=lambda **k: rec.append(k),
                classifier=lambda req, **k: type("R", (), {"domain": "clear"})(), ledger=led)
    base = [{"type": "user", "message": {"role": "user", "content": "x"}, "sessionId": "SID-9"}]
    seg1 = [
        {"type": "user", "message": {"role": "user", "content": "lee"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "3 passed", "is_error": False}]}}]
    p = tmp_path / "g.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in base + seg1), encoding="utf-8")
    r1 = S.ingest_session(p, **args)
    seg2 = [
        {"type": "user", "message": {"role": "user", "content": "otra"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "input": {}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "5 passed", "is_error": False}]}}]
    p.write_text("\n".join(json.dumps(e) for e in base + seg1 + seg2), encoding="utf-8")
    r2 = S.ingest_session(p, **args)
    assert r1.recorded == 1 and r2.recorded == 1   # cada uno solo su segmento nuevo
    assert len(rec) == 2                            # 'lee' NO se re-registro
    r3 = S.ingest_session(p, **args)               # sin crecimiento -> nada
    assert r3.already_ingested and len(rec) == 2


def test_recorder_failure_does_not_abort_or_double_count(tmp_path):
    # mmorch verify T5: si recorder() tira, el loop sigue y el ledger se escribe igual,
    # asi un re-ingest NO reprocesa (sin doble-conteo).
    def _boom(**k):
        raise RuntimeError("sink down")
    led = tmp_path / "l.txt"
    args = dict(recorder=_boom,
                classifier=lambda req, **k: type("R", (), {"domain": "clear"})(), ledger=led)
    p = _calib_session(tmp_path)
    rep = S.ingest_session(p, **args)
    assert rep.recorder_failed == 1 and rep.recorded == 0
    assert led.exists()                               # ledger escrito pese al fallo
    rep2 = S.ingest_session(p, **args)
    assert rep2.already_ingested                      # no reprocesa


def test_resolve_latest_skips_active_session(tmp_path, monkeypatch):
    # mmorch verify T5: la sesion activa (modificada recien) no se ingiere.
    import os
    proj = tmp_path / ".claude" / "projects" / "p"
    proj.mkdir(parents=True)
    active = proj / "active.jsonl"; active.write_text("{}", encoding="utf-8")
    settled = proj / "settled.jsonl"; settled.write_text("{}", encoding="utf-8")
    now = __import__("time").time()
    os.utime(active, (now, now))                      # recien modificada (activa)
    os.utime(settled, (now - 600, now - 600))         # vieja (settled)
    monkeypatch.setattr(S.Path, "home", staticmethod(lambda: tmp_path))
    assert S._resolve_latest(cooldown_s=120).name == "settled.jsonl"
