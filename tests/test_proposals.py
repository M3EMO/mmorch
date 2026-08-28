"""Tests F2 propuesta (contrato .scratch/loop-cerrado/spec.md)."""

import json
from pathlib import Path

from mmorch.iohelpers import atomic_write_json
from mmorch.proposals import compose_cards, pick_card


def match(project="proj1", score=0.9, status="pendiente", card=None,
          shown=0, cited=None, note="note1.md"):
    m = {"note_path": f"/vault/{note}", "project": project, "score": score,
         "justification": "aplica al ingest", "cited_file": cited,
         "strong": True, "status": status, "shown_count": shown,
         "id": f"/vault/{note}|{project}"}
    if card is not None:
        m["card"] = card
    return m


def write_state(logs_dir: Path, matches):
    logs_dir.mkdir(parents=True, exist_ok=True)
    by_project = {}
    for m in matches:
        by_project.setdefault(m["project"], []).append(m)
    atomic_write_json(logs_dir / "adjudications.json",
                      {"pairs": {}, "by_project": by_project})


def read_state(logs_dir: Path):
    return json.loads((logs_dir / "adjudications.json").read_text(encoding="utf-8"))


def test_compose_writes_card_to_best_pending(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match(score=0.75), match(score=0.9, note="note2.md")])
    result = compose_cards(logs_dir=str(logs))
    assert result == {"cards": 1}
    entries = read_state(logs)["by_project"]["proj1"]
    carded = [m for m in entries if "card" in m]
    assert len(carded) == 1
    assert carded[0]["note_path"].endswith("note2.md")  # el de mayor score


def test_card_template_contains_dale_and_score(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match(score=0.83)])
    compose_cards(logs_dir=str(logs))
    card = read_state(logs)["by_project"]["proj1"][0]["card"]
    assert '"dale"' in card and "0.83" in card and "proj1" in card


def test_cited_file_only_when_present(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match(cited="scripts/x.py")])
    compose_cards(logs_dir=str(logs))
    card = read_state(logs)["by_project"]["proj1"][0]["card"]
    assert "Cita: scripts/x.py" in card

    logs2 = tmp_path / "logs2"
    write_state(logs2, [match()])
    compose_cards(logs_dir=str(logs2))
    assert "Cita:" not in read_state(logs2)["by_project"]["proj1"][0]["card"]


def test_compose_skips_when_loop_paused(tmp_path):
    logs = tmp_path / "logs"
    write_state(logs, [match()])
    (logs / "loop_paused").touch()
    assert compose_cards(logs_dir=str(logs)) == {"skipped": True}
    assert "card" not in read_state(logs)["by_project"]["proj1"][0]


def make_projects(tmp_path):
    outer = tmp_path / "repos"
    inner = outer / "proj1"
    inner.mkdir(parents=True)
    return {"outer": str(outer), "proj1": str(inner)}


def test_pick_resolves_project_by_longest_prefix(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    write_state(logs, [match(project="proj1", card="tarjeta proj1"),
                       match(project="outer", card="tarjeta outer")])
    card = pick_card(projects["proj1"], projects, logs_dir=str(logs))
    assert card == "tarjeta proj1"  # gana el prefijo mas largo


def test_pick_none_when_no_project_matches(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    write_state(logs, [match(card="t")])
    assert pick_card(str(tmp_path / "otro"), projects, logs_dir=str(logs)) is None


def test_pick_respects_shown_count_limit(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    write_state(logs, [match(card="t", shown=5)])
    assert pick_card(projects["proj1"], projects, logs_dir=str(logs)) is None


def test_pick_increments_shown_count_persisted(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    write_state(logs, [match(card="t", shown=1)])
    assert pick_card(projects["proj1"], projects, logs_dir=str(logs)) == "t"
    assert read_state(logs)["by_project"]["proj1"][0]["shown_count"] == 2


def test_pick_none_without_carded_pending(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    write_state(logs, [match()])  # sin card
    assert pick_card(projects["proj1"], projects, logs_dir=str(logs)) is None


def test_pick_fail_open_on_corrupt_json(tmp_path):
    projects = make_projects(tmp_path)
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "adjudications.json").write_text("{corrupto", encoding="utf-8")
    assert pick_card(projects["proj1"], projects, logs_dir=str(logs)) is None
