"""Tests F5 loop_nightly (contrato .scratch/loop-cerrado/spec.md)."""

import json

import mmorch.loop_nightly as ln
from mmorch.iohelpers import atomic_write_json


class FakeGen:
    def propose(self, payload):
        if "lente" in payload:
            return {"gist": None, "justification": "nada nuevo"}
        return {"score": 0.9, "justification": "aplica", "cited_file": None}


class FakeVer:
    def refute(self, payload):
        return {"refuted": False, "reason": ""}


def make_repo(tmp_path):
    (tmp_path / "logs").mkdir()
    (tmp_path / "vault" / "research").mkdir(parents=True)
    (tmp_path / "vault" / "roadmaps").mkdir()
    return tmp_path


def run(repo, **kw):
    kw.setdefault("generator", FakeGen())
    kw.setdefault("verifier", FakeVer())
    kw.setdefault("record_fn", lambda *a, **k: None)
    kw.setdefault("now_ts", 1000.0)
    return ln.run_idea_loop(repo_dir=str(repo), today="2026-08-14", **kw)


def test_loop_paused_skips_everything(tmp_path):
    repo = make_repo(tmp_path)
    (repo / "logs" / "loop_paused").touch()
    assert run(repo) == {"skipped": "paused"}
    assert not (repo / "logs" / "loop_budget.json").exists()


def test_budget_exhausted_skips(tmp_path):
    repo = make_repo(tmp_path)
    atomic_write_json(repo / "logs" / "loop_budget.json",
                      {"month": "2026-08", "calls": ln.CAP_CALLS_PER_MONTH})
    assert run(repo) == {"skipped": "budget"}


def test_budget_old_month_resets(tmp_path):
    repo = make_repo(tmp_path)
    atomic_write_json(repo / "logs" / "loop_budget.json",
                      {"month": "2026-07", "calls": ln.CAP_CALLS_PER_MONTH})
    result = run(repo)
    assert "skipped" not in result
    budget = json.loads((repo / "logs" / "loop_budget.json").read_text())
    assert budget["month"] == "2026-08" and budget["calls"] == 40
    assert budget.get("usd", 0.0) == 0.0  # reset mensual arranca la plata en 0


def test_budget_usd_cap_skips(tmp_path):
    repo = make_repo(tmp_path)
    atomic_write_json(repo / "logs" / "loop_budget.json",
                      {"month": "2026-08", "calls": 0, "usd": 3.5})
    assert run(repo) == {"skipped": "budget"}


def test_steps_run_and_state_updated(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    result = run(repo)
    assert result["errors"] == []
    for name in ("expire_ignored", "expire_candidates", "detect_promotions",
                 "adjudicate", "candidatas", "compose_cards"):
        assert name in result["steps"], name
    state = json.loads((repo / "logs" / "loop_state.json").read_text())
    assert state == {"last_run_ts": 1000.0}


def test_step_crash_is_isolated(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.outcomes.expire_ignored",
                        lambda **k: (_ for _ in ()).throw(RuntimeError("boom")))
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    result = run(repo)
    assert result["steps"]["expire_ignored"] is None
    assert any("expire_ignored" in e and "boom" in e for e in result["errors"])
    assert result["steps"]["compose_cards"] is not None  # los demas corrieron


def test_metrics_count_by_status(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    def m(n, status):
        return {"note_path": f"/v/{n}.md", "project": "p", "score": 0.9,
                "strong": True, "status": status, "shown_count": 0,
                "justification": "j", "cited_file": None, "id": f"/v/{n}.md|p"}
    pairs = {f"/v/{n}.md|p": {"hash": "h", "result": m(n, st)}
             for n, st in (("a", "pendiente"), ("b", "aceptada"),
                           ("c", "pendiente"))}
    atomic_write_json(repo / "logs" / "adjudications.json",
                      {"pairs": pairs, "by_project": {}})
    result = run(repo)
    assert result["metrics"]["por_status"] == {"pendiente": 2, "aceptada": 1}


def test_adjudication_runs_over_real_note(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    proj = tmp_path / "proyecto-x"
    proj.mkdir()
    monkeypatch.setattr("mmorch.projects._load",
                        lambda *a, **k: {"proyecto-x": str(proj)})
    (repo / "vault" / "research" / "nota.md").write_text(
        "---\ntitle: t\n---\nidea util", encoding="utf-8")
    result = run(repo)
    assert result["steps"]["adjudicate"]["judged"] == 1
    adj = json.loads((repo / "logs" / "adjudications.json").read_text(
        encoding="utf-8"))
    assert adj["by_project"]["proyecto-x"][0]["strong"] is True
    # compose_cards corrio despues: el strong match tiene card
    assert "card" in adj["by_project"]["proyecto-x"][0]


def test_no_fuel_no_candidates(tmp_path, monkeypatch):
    repo = make_repo(tmp_path)
    monkeypatch.setattr("mmorch.projects._load", lambda *a, **k: {})
    atomic_write_json(repo / "logs" / "loop_state.json",
                      {"last_run_ts": 9999999999.0})
    result = run(repo)
    assert result["steps"]["candidatas"] == {"nuevas": 0, "sin_fuel": True}


def test_apply_focus_whitelist_and_determinism(tmp_path):
    logs = tmp_path / "logs"
    logs.mkdir()
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "mmorch" / "feedback.py").write_text("x", encoding="utf-8")
    # nombra un modulo real -> focus; nombra uno inexistente -> focus vacio
    f1 = ln.apply_focus({"foco_sugerido": "endurecer feedback.py ya",
                         "fecha": "2026-08-15"}, logs_dir=str(logs))
    assert f1["hardening_module"] == "mmorch/feedback.py"
    f2 = ln.apply_focus({"foco_sugerido": "arreglar inexistente.py"},
                        logs_dir=str(logs))
    assert f2 == {}


def test_summarize_record_cubre_todos_los_subsistemas():
    """El corte crudo a 1200 chars perdia SIEMPRE los mismos campos (los
    ultimos del dict). El resumen debe traer una linea por cada uno."""
    rec = {"ts": 1,
           "evolve": {"opened": [], "findings": 8},
           "project_health": {"errors": ["Portfolio: TimeoutExpired algo"]},
           "merge_train": {"error": "TimeoutExpired: pytest tardo demasiado"},
           "smoke": {"ok": 13, "total": 13, "fails": []},
           "auto_repair": {"skipped": "sin errores detectados"}}
    resumen = ln._summarize_record(rec)
    assert "ts" not in resumen
    for k in ("evolve", "project_health", "merge_train", "smoke", "auto_repair"):
        assert f"- {k} [" in resumen, f"{k} no aparece en el resumen"
    assert "[ERROR]" in resumen and "TimeoutExpired" in resumen
    assert "[skip]" in resumen


def test_summarize_record_respeta_presupuesto_por_clave():
    rec = {"ts": 1, "algo": {"detalle": "x" * 500}}
    resumen = ln._summarize_record(rec, per_key=50)
    linea = [ln_ for ln_ in resumen.splitlines() if "algo" in ln_][0]
    assert len(linea) < 80


def test_facts_cuenta_corridas_reales_y_rachas():
    """Las cifras de la reflexion tienen que salir de aca, no de su propia
    prosa anterior (medido: 6 -> 35+ 'noches' en 7 dias reales)."""
    from mmorch.loop_nightly import _facts
    recs = [{"ts": 0, "evolve": {"opened": []}},
            {"ts": 0, "evolve": {"skipped": "nada"}},
            {"ts": 0, "evolve": {"skipped": "nada"}}]
    out = _facts(recs)
    assert "corridas registradas: 3" in out
    assert "evolve: 2 corridas consecutivas sin [ok]" in out


def test_facts_subsistema_ausente_corta_la_racha():
    from mmorch.loop_nightly import _facts
    recs = [{"ts": 0}, {"ts": 0, "slim": {"skipped": "x"}}]   # noche 1 sin slim
    assert "slim: 1 corridas consecutivas" in _facts(recs)


# --- ruteo de modelo y novedad: 8/8 mutantes sobrevivian a este archivo ------ #
def _capturar_modelo(monkeypatch):
    """Devuelve una lista donde queda el modelo con el que se llamo a gated_json."""
    import mmorch.schema as sch
    visto = []

    def fake(mdl, msgs, **kw):
        visto.append(mdl)
        return {}

    monkeypatch.setattr(sch, "gated_json", fake)
    return visto


def test_llm_json_default_es_el_generador(monkeypatch):
    from mmorch.config import DEFAULT_GENERATOR
    visto = _capturar_modelo(monkeypatch)
    ln._llm_json("x", schema=ln._DESC_SCHEMA)
    assert visto == [DEFAULT_GENERATOR]


def test_llm_json_refutacion_va_cross_family(monkeypatch):
    """El refutador tiene que ser de OTRA familia — invariante del sistema."""
    from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER
    visto = _capturar_modelo(monkeypatch)
    ln._llm_json("x", schema=ln._REFUTE_SCHEMA)
    assert visto == [DEFAULT_VERIFIER] and DEFAULT_VERIFIER != DEFAULT_GENERATOR


def test_llm_json_modelo_explicito_manda(monkeypatch):
    visto = _capturar_modelo(monkeypatch)
    ln._llm_json("x", schema=ln._DESC_SCHEMA, model="modelo-pedido")
    assert visto == ["modelo-pedido"]


def test_novelty_repetido_es_cero_y_nuevo_es_uno():
    texto = "grafo de topics con bursts de arxiv"
    assert ln._novelty(texto, []) == 1.0            # nada visto -> todo nuevo
    assert ln._novelty(texto, [texto]) == 0.0       # identico -> nada nuevo


def test_novelty_parcial_queda_entre_los_extremos():
    n = ln._novelty("grafo de topics ajenos",
                    ["grafo de repos propios", "cosa completamente distinta"])
    assert 0.0 < n < 1.0


def test_describe_projects_reporta_pendientes(tmp_path, monkeypatch):
    """`described` es incremental; sin `pendientes` se leia como estancamiento."""
    logs = tmp_path / "logs"
    logs.mkdir()
    atomic_write_json(logs / "projects_meta.json",
                      {"ya": {"desc": "cubierto", "updated": "2026-01-01"}})
    monkeypatch.setattr(ln, "_llm_json", lambda *a, **k: {"desc": "nuevo"})
    proj = tmp_path / "nuevo"
    proj.mkdir()
    r = ln.describe_projects({"ya": str(tmp_path), "nuevo": str(proj)},
                             logs_dir=str(logs), today="2026-08-24")
    assert r == {"described": 1, "total": 2, "pendientes": 0}
