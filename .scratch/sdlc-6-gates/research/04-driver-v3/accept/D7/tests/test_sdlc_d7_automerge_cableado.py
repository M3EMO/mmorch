"""D7 (reparacion: `try_automerge` sin caller vivo -> cablear, decision del usuario 2026-09-14).

`auto_apply._finish_merge` delega el merge del carril verde a `mmorch.automerge.try_automerge`
(semaforo + ledger append-only en logs/automerge_ledger.jsonl). El carril amarillo acotado
(`yellow_bounded`) conserva el merge directo de auto_apply. Un rechazo del semaforo detiene la promocion.
Cero git real: try_automerge se parchea; el store es real en tmp.
"""
import mmorch.auto_apply as AA
from mmorch.promotion import PromotionStore


class _Runtime:
    path = "."

    def clean(self):
        return True

    def head(self):
        return "base"


def _candidate(tmp_path, zone="green"):
    store = PromotionStore(tmp_path / "state")
    store.begin(branch="candidate", base_sha="base", diff_hash="d", promotion_id="p1", now=1.0,
                evidence={"zone": zone},
                fields={"baseline": {}, "samples": [], "preflight_checks": {}, "zone": zone})
    return store


def test_R1_finish_merge_usa_try_automerge_en_verde(tmp_path, monkeypatch):
    calls = []

    def fake(repo, branch, *, base, source=""):
        calls.append((branch, base, source))
        return {"merged": True, "zone": "green", "veredicto": "merged", "merge_sha": "abc123", "checks": []}

    monkeypatch.setattr(AA, "try_automerge", fake)
    store = _candidate(tmp_path)
    state = AA._finish_merge(store, _Runtime(), now=2.0)
    assert calls == [("candidate", "base", "auto_apply")], calls
    assert state["status"] == "observing", state
    assert state["merge_sha"] == "abc123", state


def test_R2_rechazo_del_semaforo_detiene_la_promocion(tmp_path, monkeypatch):
    monkeypatch.setattr(AA, "try_automerge", lambda repo, branch, *, base, source="": {
        "merged": False, "zone": "red", "veredicto": "rechazado_red", "reason": "path rojo: GOAL.md",
        "merge_sha": None, "checks": []})
    store = _candidate(tmp_path)
    state = AA._finish_merge(store, _Runtime(), now=2.0)
    assert state["status"] == "halted", state
    assert "rechazado_red" in state.get("halt_reason", ""), state


def test_R3_merge_sha_conocido_no_llama_al_semaforo(tmp_path, monkeypatch):
    monkeypatch.setattr(AA, "try_automerge", lambda *a, **k: (_ for _ in ()).throw(AssertionError("no debe llamarse")))
    store = _candidate(tmp_path)
    state = AA._finish_merge(store, _Runtime(), now=2.0, merge_sha="ya-mergeado")
    assert state["status"] == "observing" and state["merge_sha"] == "ya-mergeado", state
