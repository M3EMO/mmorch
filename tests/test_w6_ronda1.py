"""W6 ronda 1 — regresiones de los defectos confirmados por los 3 verificadores:
goal fallback en HOME fresco (AT-16), metrics envenenado (D-ADV1), beat digest
(AT-19), /health por proveedor (AT-20), system_check encadenado (AT-21),
evolve_red con porque (AT-29), y el par job-muerto-en-fase / resume-doble
(observacion adversarial sobre D3)."""
import json
import os
import pathlib
import subprocess
import sys
import threading

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

ROOT = pathlib.Path(__file__).resolve().parent.parent
PY = sys.executable


# ---- F1: GOAL.md con MMORCH_HOME fresco (AT-16) -------------------------------
def test_goal_fallback_con_mmorch_home_fresco(tmp_path):
    """Un HOME fresco sin GOAL.md mataba evaluate() con FileNotFoundError (4 tests
    de test_evolve_motor rojos). El contrato es policy del repo: fallback al checkout.
    Subprocess porque la resolucion es a import-time (contrato de paths.py)."""
    env = {**os.environ, "MMORCH_HOME": str(tmp_path)}
    r = subprocess.run(
        [PY, "-c", "from mmorch.goal import load_goal; "
                   "assert load_goal().strip(), 'GOAL vacio'; print('ok')"],
        capture_output=True, text=True, env=env, cwd=str(ROOT), timeout=120)
    assert r.returncode == 0 and "ok" in r.stdout, r.stderr[-800:]


# ---- F2: metrics.jsonl envenenado (D-ADV1) ------------------------------------
def test_summary_con_cost_usd_no_numerico(monkeypatch):
    """Una linea JSON valida con cost_usd string brickeaba summary() (TypeError)
    y con el GET /state entero."""
    import mmorch.metrics as M
    monkeypatch.setattr(M, "read_events", lambda: [
        {"model": "x", "family": "f", "cost_usd": "not-a-number"},
        {"model": "x", "family": "f", "cost_usd": 0.5}])
    s = M.summary()
    assert s["calls"] == 2 and s["total_cost_usd"] == 0.5
    assert s["cost_by_family"]["f"] == 0.5


def test_cache_stats_con_tokens_no_numericos(monkeypatch):
    import mmorch.metrics as M
    monkeypatch.setattr(M, "read_events", lambda: [
        {"model": "x", "in_tokens": "muchos"},                       # veneno: se saltea
        {"model": "x", "in_tokens": 10, "extra": {"cached_tokens": "z"}}])
    st = M.cache_stats()
    assert st["by_model"]["x"]["in_tokens"] == 10
    assert st["by_model"]["x"]["cached_tokens"] == 0


# ---- F3: emisor de beat("digest") en el camino ACTIVO (AT-19) -----------------
def test_nightly_emite_beat_digest():
    """health.EXPECTATIONS declara 'digest' => DEBE tener emisor en el camino que
    corre de verdad (nightly.py); tenerlo solo en loop_nightly (inactivo) dio
    healthy=False cronico. Truth-test de fuente (el camino completo necesita LLM)."""
    src = (ROOT / "mmorch" / "nightly.py").read_text(encoding="utf-8")
    assert '_beat("digest"' in src, "nightly.py perdio el emisor de beat('digest')"
    from mmorch.health import EXPECTATIONS
    assert "digest" in EXPECTATIONS


# ---- F4: /health con estado por proveedor (AT-20) -----------------------------
def test_breaker_snapshot_expone_abierto():
    import mmorch.providers as P
    P._BREAKERS.pop("modelo-w6", None)
    try:
        for _ in range(3):
            P._breaker_record("modelo-w6", ok=False)
        snap = P.breaker_snapshot()
        assert snap["modelo-w6"]["open"] is True
        assert snap["modelo-w6"]["cooldown_left_s"] > 0
    finally:
        P._BREAKERS.pop("modelo-w6", None)


def _client(monkeypatch, token="secret"):
    import tempfile
    monkeypatch.setenv("MMORCH_SERVER_TOKEN", token)
    monkeypatch.setenv("MMORCH_HOME", tempfile.mkdtemp())
    import importlib
    import mmorch.server as S
    importlib.reload(S)
    from starlette.testclient import TestClient
    return S, TestClient(S.build_app())


H = {"X-Token": "secret"}


def test_health_endpoint_incluye_proveedores(monkeypatch):
    """El 503 sin 'que proveedor' no dejaba distinguir caido de sin-saldo."""
    S, c = _client(monkeypatch)
    j = c.get("/health").json()          # sin auth por diseño; status 200 o 503
    assert "providers" in j
    assert "breakers" in j["providers"] and "error_rates" in j["providers"]


# ---- F5: system_check --fast sigue barato; el default encadena (AT-21) --------
def test_system_check_fast_no_corre_suite(tmp_path):
    env = {**os.environ, "MMORCH_HOME": str(tmp_path)}
    r = subprocess.run([PY, "scripts/system_check.py", "--fast"],
                       capture_output=True, text=True, env=env, cwd=str(ROOT),
                       timeout=180)
    out = json.loads(r.stdout)
    assert "health" in out and "goal" in out and "budget" in out
    assert "pytest" not in out and "gates" not in out and "smoke" not in out
    # HOME fresco: sin beats ni GOAL.hash => veredicto honesto = rojo
    assert r.returncode == 1 and out["ok"] is False


def test_system_check_default_declara_cadena_completa():
    """AT-21 lo exige literal: ruff+mypy (gates.py) + pytest + smoke + health.
    Correr la cadena real aca seria la suite adentro de la suite: se verifica
    que el codigo la construya, y el --fast (arriba) cubre la mitad viva."""
    src = (ROOT / "scripts" / "system_check.py").read_text(encoding="utf-8")
    for paso in ("scripts/gates.py", "pytest", "scripts/smoke.py"):
        assert paso in src, f"system_check perdio el paso {paso}"


# ---- F6: evolve_red.jsonl con zone + reason (AT-29) ---------------------------
def test_evolve_red_linea_con_zone_y_reason(tmp_path, monkeypatch):
    import mmorch.evolve as E

    class _R:
        returncode = 1
        stdout = ""
    monkeypatch.setattr(E, "_git", lambda *a, cwd: _R())
    c = E.snapshot_change("a.py", "def a(): return 1", "fix a", root=tmp_path)
    r = E.coordinated_evolve_round(
        [c], root=tmp_path, path=tmp_path / "pr_state.json",
        sandbox_fn=lambda ch: {"ok": False, "branch": None, "fitness": {},
                               "change_id": ch.id},
        pr_fn=lambda b, title: {"pushed": True},
        aligned_fn=lambda ch: None, fitness_fn=lambda ch: {"ok": True})
    assert r["red"] == ["a.py"], r
    line = json.loads((tmp_path / "logs" / "evolve_red.jsonl")
                      .read_text(encoding="utf-8").splitlines()[-1])
    assert line["zone"] == "green"
    assert line["reason"] == "sandbox_fail_sin_detalle"


def test_evolve_red_reason_prefiere_suite_roja(tmp_path, monkeypatch):
    import mmorch.evolve as E

    class _R:
        returncode = 1
        stdout = ""
    monkeypatch.setattr(E, "_git", lambda *a, cwd: _R())
    c = E.snapshot_change("a.py", "x=1", "fix", root=tmp_path)
    E.coordinated_evolve_round(
        [c], root=tmp_path, path=tmp_path / "pr_state.json",
        sandbox_fn=lambda ch: {"ok": False, "branch": None, "change_id": ch.id,
                               "fitness": {"failed": 3, "rc": 1, "detail": "..."}},
        pr_fn=lambda b, title: {}, aligned_fn=lambda ch: None,
        fitness_fn=lambda ch: {"ok": True})
    line = json.loads((tmp_path / "logs" / "evolve_red.jsonl")
                      .read_text(encoding="utf-8").splitlines()[-1])
    assert "3 failed" in line["reason"]


# ---- F7/F8: job muerto en fase + guard de resume (observacion adversarial D3) --
def test_rubric_drive_excepcion_deja_status_error(monkeypatch):
    """BudgetExceeded a mitad de run dejaba status='executor' (fase) — el cliente
    veia el job muerto como vivo y el guard de resume lo dejaba doble-correr."""
    import tempfile
    monkeypatch.setenv("MMORCH_HOME", tempfile.mkdtemp())   # no ensuciar jobs.jsonl real
    import mmorch.rubric_loop as RL
    import mmorch.workflow_store as WS
    import mmorch.server_engine as SE
    from mmorch.server_core import _JOBS, _JOBS_LOCK

    def _boom(state):
        raise RuntimeError("BudgetExceeded simulado")
    monkeypatch.setattr(RL, "next_action", _boom)
    monkeypatch.setattr(WS, "checkpoint_history", lambda jid: [])
    state = {"gen_model": "g", "judge_model": "j", "phase": "executor"}
    with _JOBS_LOCK:
        _JOBS["jw6f7"] = {"status": "executor", "kind": "rubric"}
    try:
        SE._rubric_drive("jw6f7", state, threading.Event())
        assert _JOBS["jw6f7"]["status"] == "error"
    finally:
        with _JOBS_LOCK:
            _JOBS.pop("jw6f7", None)


def test_state_resumable_false_para_job_vivo_en_fase(monkeypatch):
    """El guard viejo (!= 'running') dejaba resumable:true a un job VIVO cuyo
    status es la fase ('executor')."""
    S, c = _client(monkeypatch)
    import mmorch.workflow_store as WS
    monkeypatch.setattr(WS, "jobs_with_checkpoints", lambda: {"jv1", "jv2"})
    monkeypatch.setattr(WS, "jobs_with_specs", lambda: {"jv1", "jv2"})
    from mmorch.server_core import _JOBS, _JOBS_LOCK
    with _JOBS_LOCK:
        _JOBS["jv1"] = {"status": "executor", "kind": "rubric"}      # vivo en fase
        _JOBS["jv2"] = {"status": "interrupted", "kind": "rubric"}   # muerto real
    try:
        jobs = c.get("/state", headers=H).json()["jobs"]
        assert jobs["jv1"]["resumable"] is False
        assert jobs["jv2"]["resumable"] is True
    finally:
        with _JOBS_LOCK:
            _JOBS.pop("jv1", None)
            _JOBS.pop("jv2", None)


def test_resume_409_para_job_vivo_en_fase(monkeypatch):
    S, c = _client(monkeypatch)
    import mmorch.workflow_store as WS
    monkeypatch.setattr(WS, "get_job_spec",
                        lambda jid: {"kind": "rubric", "spec": {"state": {"phase": "executor"}}})
    monkeypatch.setattr(WS, "checkpoint_latest", lambda jid: {"step": 1})
    from mmorch.server_core import _JOBS, _JOBS_LOCK
    with _JOBS_LOCK:
        _JOBS["jv3"] = {"status": "executor", "kind": "rubric"}
    try:
        r = c.post("/jobs/jv3/resume", headers=H)
        assert r.status_code == 409, r.text
    finally:
        with _JOBS_LOCK:
            _JOBS.pop("jv3", None)
