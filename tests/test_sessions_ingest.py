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
