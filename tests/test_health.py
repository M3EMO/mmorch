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
    # operacion normal = los 3 componentes declarados laten (todos tienen
    # emisor real); recien ahi healthy=True es alcanzable
    now_ts = 1000.0

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now_ts - 50)
    health.beat("digest", logs_dir=str(tmp_path), now_ts=now_ts - 50)

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts)

    assert result["healthy"] is True
    assert result["check"]["alive"] == ["digest", "nightly", "server"]
    assert result["check"]["never"] == []
    assert result["errors"]["server_err_tail"] == []
    assert result["errors"]["nightly_errors"] == {}
    assert result["errors"]["idea_loop_errors"] == []


def test_report_healthy_false_never(tmp_path):
    # componente declarado que jamas latio = rojo (antes "never" no pesaba en
    # healthy y la alarma cronica se entrenaba a ignorarse)
    now_ts = 1000.0

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts)

    assert result["healthy"] is False
    assert result["check"]["never"] == ["digest", "nightly"]


def test_report_healthy_false_dead(tmp_path):
    now_ts = 1000.0

    # limite de server = 900 -> latido hace 1000s esta vencido (overdue 100)
    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 1000)

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts + 1000)

    assert result["healthy"] is False
    assert len(result["check"]["dead"]) == 1


def test_report_healthy_false_errors(tmp_path):
    # los 3 laten para aislar la causa: el rojo viene SOLO del *_error
    now_ts = 1000.0

    health.beat("server", logs_dir=str(tmp_path), now_ts=now_ts - 50)
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now_ts - 50)
    health.beat("digest", logs_dir=str(tmp_path), now_ts=now_ts - 50)

    (tmp_path / "nightly.jsonl").write_text(
        json.dumps({"ts": 900.0, "digest_error": "failed"}) + "\n"
    )

    result = health.report(logs_dir=str(tmp_path), now_ts=now_ts)

    assert result["healthy"] is False
    assert result["errors"]["nightly_errors"] == {"digest_error": "failed"}


def test_check_projects_classifies(tmp_path):
    a = tmp_path / "a"; (a / "tests").mkdir(parents=True)
    b = tmp_path / "b"; (b / "tests").mkdir(parents=True)
    c = tmp_path / "c"; c.mkdir()  # sin tests/
    projects = {"a": str(a), "b": str(b), "c": str(c)}
    result = health.check_projects(projects, run_fn=lambda p: p == str(a))
    assert result == {"ok": ["a"], "failing": ["b"], "sin_tests": ["c"],
                      "errors": []}


def test_check_projects_fail_soft(tmp_path):
    a = tmp_path / "a"; (a / "tests").mkdir(parents=True)
    def boom(p):
        raise RuntimeError("venv roto")
    result = health.check_projects({"a": str(a)}, run_fn=boom)
    assert result["failing"] == [] and result["ok"] == []
    assert "venv roto" in result["errors"][0]


def test_report_incluye_silent_errors_recientes(tmp_path):
    """report() debe sumar un resumen de silent_errors.jsonl sin fusionar
    su detalle — el subsistema que lo escribio sigue siendo el dueño.

    Fecha RELATIVA a hoy, no hardcodeada: la version con '2026-08-19' fija
    paso el dia que se escribio y se pudrio sola 48h despues — mato los 6
    sandboxes del estreno de evolve (todos rojos por ESTE test, no por sus
    parches). Un test con fecha absoluta es una bomba de tiempo literal."""
    import datetime
    from mmorch.health import report
    logs = tmp_path
    hoy = datetime.date.today().isoformat()
    (logs / "silent_errors.jsonl").write_text(
        '{"fecha": "%s", "source": "merge_train", "detail": "timeout"}\n'
        '{"fecha": "2020-01-01", "source": "viejo", "detail": "hace anios"}\n'
        % hoy,
        encoding="utf-8")
    r = report(logs_dir=str(logs))
    assert r["silent_errors_48h"] == 1
    assert r["silent_errors_sample"][0]["source"] == "merge_train"


# --- nightly_watchdog (W4.4): dead-man VISIBLE mas alla de health.report ---

def test_watchdog_nightly_vencido_grita_y_persiste_episodio(tmp_path, capsys):
    now = 1_000_000.0
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now - 27 * 3600)
    episodios = []
    f = health.nightly_watchdog(
        logs_dir=str(tmp_path), now_ts=now,
        write_episode_fn=lambda scope, kind, msg: episodios.append((scope, kind, msg)))
    assert f is not None and f["status"] == "dead" and f["episode_written"]
    # el grito va por stderr (stdout es protocolo MCP / JSON del CLI)
    err = capsys.readouterr().err
    assert "WATCHDOG" in err and "mmorch-nightly" in err
    assert episodios and episodios[0][1] == "watchdog"


def test_watchdog_episodio_una_vez_por_dia_pero_grita_siempre(tmp_path, capsys):
    now = 1_000_000.0
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now - 27 * 3600)
    episodios = []
    fn = lambda *a: episodios.append(a)  # noqa: E731
    f1 = health.nightly_watchdog(logs_dir=str(tmp_path), now_ts=now, write_episode_fn=fn)
    f2 = health.nightly_watchdog(logs_dir=str(tmp_path), now_ts=now, write_episode_fn=fn)
    assert f1["episode_written"] and not f2["episode_written"]
    assert len(episodios) == 1
    # el segundo arranque igual grita: la señal visible no se deduplica
    assert capsys.readouterr().err.count("WATCHDOG") == 2


def test_watchdog_nightly_vivo_devuelve_none_y_no_grita(tmp_path, capsys):
    now = 1_000_000.0
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=now - 3600)
    assert health.nightly_watchdog(logs_dir=str(tmp_path), now_ts=now) is None
    assert capsys.readouterr().err == ""


def test_watchdog_nightly_jamas_latio_tambien_es_rojo(tmp_path, capsys):
    f = health.nightly_watchdog(logs_dir=str(tmp_path), now_ts=1_000_000.0,
                                write_episode_fn=lambda *a: None)
    assert f is not None and f["status"] == "never"
    assert "NUNCA" in capsys.readouterr().err


def test_watchdog_fail_open_si_el_episodio_revienta(tmp_path, capsys):
    """El watchdog jamas frena el arranque de un entry point: si la memoria
    episodica (DuckDB) esta rota, el grito visible igual sale."""
    def boom(*a):
        raise RuntimeError("duckdb lockeada")
    health.beat("nightly", logs_dir=str(tmp_path), now_ts=1_000_000.0 - 30 * 3600)
    f = health.nightly_watchdog(logs_dir=str(tmp_path), now_ts=1_000_000.0,
                                write_episode_fn=boom)
    assert f is not None and not f["episode_written"]
    assert "WATCHDOG" in capsys.readouterr().err
