import json


from mmorch import health


def test_beat_appends_valid_json(tmp_path):
    now_ts = 1000.0
    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts, detail="ok")

    lines = (tmp_path / "health.jsonl").read_text().strip().splitlines()
    assert len(lines) == 1
    record = json.loads(lines[0])
    assert record == {"component": "server", "ts": now_ts, "detail": "ok"}


def test_beat_fail_open_with_invalid_logs_dir(tmp_path):
    # Should not raise even with invalid logs_dir
    health.beat("server", logs_dir=str(tmp_path / "nonexistent" / "dir"), now_ts=1000.0)


def test_check_classifies_alive_dead_never(tmp_path):
    now_ts = 1000.0
    expectations = {"server": 100, "nightly": 200, "digest": 300}

    # server: alive (last beat 50s ago)
    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)
    # nightly: dead (last beat 250s ago, limit 200)
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now_ts - 250)
    # digest: never (no beats)

    result = health.check(logs_dir=str(tmp_path), now_ts=now_ts, expectations=expectations)

    assert result["alive"] == ["server"]
    assert result["never"] == ["digest"]
    assert len(result["dead"]) == 1
    dead = result["dead"][0]
    assert dead["component"] == "nightly"
    assert dead["last_ts"] == now_ts - 250
    assert dead["overdue_s"] == 50


def test_check_overdue_s_correct(tmp_path):
    now_ts = 5000.0
    expectations = {"server": 100}

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 150)

    result = health.check(logs_dir=str(tmp_path), now_ts=now_ts, expectations=expectations)

    assert result["dead"][0]["overdue_s"] == 50


def test_check_ignores_corrupt_lines(tmp_path):
    now_ts = 1000.0
    expectations = {"server": 100}

    log_file = tmp_path / "health.jsonl"
    log_file.write_text(
        "not json\n"
        '{"component": "server", "ts": 900.0, "detail": "ok"}\n'
        "also not json\n"
    )

    result = health.check(logs_dir=str(tmp_path), now_ts=now_ts, expectations=expectations)

    assert result["alive"] == ["server"]
    assert result["dead"] == []
    assert result["never"] == []


def test_scrape_errors_collects_all_sources(tmp_path):
    # server_err_tail
    (tmp_path / "server_forever.err").write_text(
        "line1\nline2\nline3\n\nline4\n"
    )

    # nightly.jsonl with multiple records, last one has errors
    nightly_records = [
        {"ts": 100.0, "ok": True},
        {
            "ts": 200.0,
            "server_err": "old error",
            "idea_loop": {"errors": ["old idea error"]},
        },
        {
            "ts": 300.0,
            "digest_error": "digest failed",
            "idea_loop": {"errors": ["idea error 1", "idea error 2"]},
        },
    ]
    (tmp_path / "nightly.jsonl").write_text(
        "\n".join(json.dumps(r) for r in nightly_records) + "\n"
    )

    result = health.scrape_errors(logs_dir=str(tmp_path), max_lines=2)

    assert result["server_err_tail"] == ["line3", "line4"]
    assert result["nightly_errors"] == {"digest_error": "digest failed"}
    assert result["idea_loop_errors"] == ["idea error 1", "idea error 2"]


def test_report_healthy_true(tmp_path):
    now_ts = 1000.0

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts)

    assert result["healthy"] is True
    assert result["check"]["alive"] == ["server"]
    assert result["errors"]["server_err_tail"] == []
    assert result["errors"]["nightly_errors"] == {}
    assert result["errors"]["idea_loop_errors"] == []


def test_report_healthy_false_dead(tmp_path):
    now_ts = 1000.0

    # limite de server = 900 -> latido hace 1000s esta vencido (overdue 100)
    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 1000)

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts + 1000)

    assert result["healthy"] is False
    assert len(result["check"]["dead"]) == 1


def test_report_healthy_false_errors(tmp_path):
    now_ts = 1000.0

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)

    (tmp_path / "nightly.jsonl").write_text(
        json.dumps({"ts": 900.0, "digest_error": "failed"}) + "\n"
    )

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts)

    assert result["healthy"] is False
    assert result["errors"]["nightly_errors"] == {"digest_error": "failed"}
