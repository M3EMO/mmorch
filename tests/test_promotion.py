import json

import pytest

from mmorch.promotion import (
    PromotionBusy,
    PromotionCorrupt,
    PromotionError,
    PromotionStore,
    transition,
)


def _begin(store, promotion_id="p1"):
    return store.begin(
        branch="candidate-1",
        base_sha="base123",
        diff_hash="diff456",
        promotion_id=promotion_id,
        now=10,
        evidence={"zone": "green"},
    )


def test_transition_graph_and_auto_action_history():
    state = {
        "schema": 1,
        "id": "p1",
        "status": "candidate",
        "branch": "b",
        "base_sha": "base",
        "diff_hash": "diff",
        "created_at": 1.0,
        "updated_at": 1.0,
        "history": [{"kind": "auto_action", "status": "candidate", "ts": 1.0, "evidence": {}}],
    }
    merged = transition(state, "merged", now=2, fields={"merge_sha": "merge1"})
    observing = transition(merged, "observing", now=3)
    accepted = transition(observing, "accepted", now=4)

    assert accepted["status"] == "accepted"
    assert accepted["merge_sha"] == "merge1"
    assert [e["status"] for e in accepted["history"]] == [
        "candidate", "merged", "observing", "accepted",
    ]
    assert all(e["kind"] == "auto_action" for e in accepted["history"])
    with pytest.raises(PromotionError, match="invalid transition"):
        transition(state, "accepted")


def test_repeating_transition_is_idempotent():
    store_state = {
        "schema": 1,
        "id": "p1",
        "status": "candidate",
        "branch": "b",
        "base_sha": "base",
        "diff_hash": "diff",
        "created_at": 1.0,
        "updated_at": 1.0,
        "history": [{"kind": "auto_action", "status": "candidate", "ts": 1.0, "evidence": {}}],
    }
    same = transition(store_state, "candidate", now=99)
    assert same == store_state
    assert len(same["history"]) == 1


def test_store_survives_process_restart_and_keeps_identity(tmp_path):
    first_process = PromotionStore(tmp_path)
    _begin(first_process)
    first_process.advance("merged", expected="candidate", fields={"merge_sha": "m1"}, now=11)

    second_process = PromotionStore(tmp_path)
    loaded = second_process.load()
    assert loaded["status"] == "merged"
    assert loaded["merge_sha"] == "m1"
    assert loaded["branch"] == "candidate-1"
    second_process.advance("observing", expected="merged", now=12)
    assert PromotionStore(tmp_path).load()["status"] == "observing"


def test_only_accepted_promotion_may_be_superseded(tmp_path):
    store = PromotionStore(tmp_path)
    _begin(store)
    with pytest.raises(PromotionBusy, match="candidate"):
        _begin(store, promotion_id="p2")

    store.advance("merged")
    store.advance("observing")
    store.advance("accepted")
    newer = _begin(store, promotion_id="p2")
    assert newer["id"] == "p2"
    assert store.load("p1")["status"] == "accepted"


def test_halted_store_cannot_restart_automatically(tmp_path):
    store = PromotionStore(tmp_path)
    _begin(store)
    store.advance("halted")
    with pytest.raises(PromotionBusy, match="halted"):
        _begin(store, promotion_id="p2")


def test_existing_lock_fails_closed(tmp_path):
    store = PromotionStore(tmp_path)
    tmp_path.mkdir(exist_ok=True)
    store.lock_path.write_text("other process", encoding="utf-8")
    with pytest.raises(PromotionBusy, match="lock exists"):
        _begin(store)
    assert not store.active_path.exists()


@pytest.mark.parametrize("which", ["pointer", "state"])
def test_corrupt_durable_state_fails_closed(tmp_path, which):
    store = PromotionStore(tmp_path)
    _begin(store)
    path = store.active_path if which == "pointer" else store.states / "p1.json"
    path.write_text("{broken", encoding="utf-8")
    with pytest.raises(PromotionCorrupt, match="unreadable"):
        store.load()


def test_changed_identity_cannot_reuse_promotion_id(tmp_path):
    store = PromotionStore(tmp_path)
    _begin(store)
    store.advance("merged")
    store.advance("observing")
    store.advance("accepted")
    store.active_path.write_text(json.dumps({"schema": 1, "id": "p1"}), encoding="utf-8")
    with pytest.raises(PromotionCorrupt, match="different identity"):
        store.begin(
            branch="other",
            base_sha="base123",
            diff_hash="diff456",
            promotion_id="p1",
        )


def test_expected_state_prevents_stale_actor(tmp_path):
    store = PromotionStore(tmp_path)
    _begin(store)
    store.advance("merged", expected="candidate")
    with pytest.raises(PromotionBusy, match="expected.*candidate.*merged"):
        store.advance("halted", expected="candidate")
    assert store.load()["status"] == "merged"
