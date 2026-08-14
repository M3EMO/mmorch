import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).parent.parent))
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from proposals import compose_cards, pick_card


def make_proposals_file(tmp_path, data):
    path = tmp_path / "proposals.json"
    path.write_text(json.dumps(data))
    return path


def make_state_file(tmp_path, data):
    path = tmp_path / "state.json"
    path.write_text(json.dumps(data))
    return path


def test_compose_cards_writes_to_best_pending_without_card(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "hello {{name}}", "score": 5},
            {"id": "p2", "template": "world", "score": 3},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    result = compose_cards(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path,
        name="dale"
    )

    assert result == 1
    data = json.loads(proposals_file.read_text())
    assert data["pending"][0]["id"] == "p2"
    assert data["pending"][0]["template"] == "world"
    assert data["pending"][0]["score"] == 3
    assert data["pending"][0]["card"] == "world"


def test_compose_cards_template_contains_dale_and_score(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "Hi {{name}}!", "score": 7},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    compose_cards(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path,
        name="dale"
    )

    data = json.loads(proposals_file.read_text())
    card = data["pending"][0]["card"]
    assert "dale" in card
    assert "7" in card


def test_compose_cards_cited_file_appears_only_if_exists(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "x", "score": 1, "cited_file": "notes.txt"},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    # Without the file existing
    compose_cards(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path,
        name="dale"
    )
    data = json.loads(proposals_file.read_text())
    assert "cited_file" not in data["pending"][0]["card"]

    # With the file existing
    (tmp_path / "notes.txt").write_text("important")
    proposals_file2 = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "x", "score": 1, "cited_file": "notes.txt"},
        ],
        "cited": []
    })
    state_file2 = make_state_file(tmp_path, {"shown_count": 0})
    compose_cards(
        proposals_file=proposals_file2,
        state_file=state_file2,
        cwd=tmp_path,
        name="dale"
    )
    data2 = json.loads(proposals_file2.read_text())
    assert "important" in data2["pending"][0]["card"]


def test_compose_cards_loop_paused_skips(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "a", "score": 5, "loop_paused": True},
            {"id": "p2", "template": "b", "score": 3},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    result = compose_cards(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path,
        name="dale"
    )

    assert result == 1
    data = json.loads(proposals_file.read_text())
    assert data["pending"][0]["id"] == "p2"
    assert data["pending"][0]["card"] == "b"


def test_pick_card_resolves_project_by_cwd_prefix_longest_wins(tmp_path):
    # Create two project dirs: one nested inside another
    outer = tmp_path / "project"
    inner = outer / "sub"
    inner.mkdir(parents=True)

    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "outer", "score": 1, "project": str(outer)},
            {"id": "p2", "template": "inner", "score": 2, "project": str(inner)},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    # cwd is the inner dir, longest prefix should win
    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=inner
    )

    assert result is not None
    assert result["id"] == "p2"


def test_pick_card_respects_shown_count_less_than_5(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "a", "score": 1},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 4})

    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path
    )

    assert result is not None
    assert result["id"] == "p1"


def test_pick_card_with_5_shown_count_returns_none(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "a", "score": 1},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 5})

    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path
    )

    assert result is None


def test_pick_card_increments_shown_count_persisted(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "a", "score": 1},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 2})

    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path
    )

    assert result is not None
    state_data = json.loads(state_file.read_text())
    assert state_data["shown_count"] == 3


def test_pick_card_no_match_returns_none(tmp_path):
    proposals_file = make_proposals_file(tmp_path, {
        "pending": [
            {"id": "p1", "template": "a", "score": 1, "project": "/nonexistent"},
        ],
        "cited": []
    })
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path
    )

    assert result is None


def test_pick_card_internal_exception_corrupt_json_returns_none(tmp_path):
    proposals_file = tmp_path / "proposals.json"
    proposals_file.write_text("{invalid json")
    state_file = make_state_file(tmp_path, {"shown_count": 0})

    result = pick_card(
        proposals_file=proposals_file,
        state_file=state_file,
        cwd=tmp_path
    )

    assert result is None
