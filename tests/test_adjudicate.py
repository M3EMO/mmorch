"""Tests F1 adjudicacion nota->proyecto (contrato .scratch/loop-cerrado/spec.md).

Fakes explicitos, sin API real. yaml/dotenv reales (estan en el venv).
"""

import json

from mmorch.adjudicate import adjudicate, run_incremental


class FakeGenerator:
    def __init__(self, response=None):
        self.response = {"score": 0.0, "justification": "j", "cited_file": None,
                         **(response or {})}

    def propose(self, payload):
        return self.response


class FakeVerifier:
    def __init__(self, response=None):
        self.response = {"refuted": False, "reason": "", **(response or {})}

    def refute(self, payload):
        return self.response


class CapturingGenerator:
    """Fake que guarda el ultimo payload recibido (para inspeccionar codegraph)."""

    def __init__(self):
        self.last_payload = None

    def propose(self, payload):
        self.last_payload = payload
        return {"score": 0.5, "justification": "capture", "cited_file": None}


def make_note(tmp_path, content="---\ntitle: nota\n---\ncuerpo"):
    notes_dir = tmp_path / "notes"
    notes_dir.mkdir(exist_ok=True)
    note_file = notes_dir / "note1.md"
    note_file.write_text(content, encoding="utf-8")
    return notes_dir, note_file


def make_project(tmp_path, name="proj1"):
    project_dir = tmp_path / "projects" / name
    project_dir.mkdir(parents=True, exist_ok=True)
    return project_dir


def note_dict(note_file):
    content = note_file.read_text(encoding="utf-8")
    return {"path": str(note_file), "content": content, "hash": "h1"}


def test_strong_match_survives_refutation(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    result = adjudicate(
        note_dict(note_file), "proj1", str(project_dir),
        FakeGenerator({"score": 0.9}), FakeVerifier({"refuted": False}),
        logs_dir=str(tmp_path / "logs"),
    )
    assert result["strong"] is True
    assert result["status"] == "pendiente"
    assert result["shown_count"] == 0
    assert result["id"]


def test_low_score_not_strong(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    result = adjudicate(
        note_dict(note_file), "proj1", str(project_dir),
        FakeGenerator({"score": 0.3}), FakeVerifier({"refuted": False}),
        logs_dir=str(tmp_path / "logs"),
    )
    assert result["strong"] is False


def test_refuted_not_strong(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    result = adjudicate(
        note_dict(note_file), "proj1", str(project_dir),
        FakeGenerator({"score": 0.9}), FakeVerifier({"refuted": True, "reason": "no aplica"}),
        logs_dir=str(tmp_path / "logs"),
    )
    assert result["strong"] is False


def test_loop_paused(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    logs_dir = tmp_path / "logs"
    logs_dir.mkdir(exist_ok=True)
    (logs_dir / "loop_paused").touch()
    result = adjudicate(
        note_dict(note_file), "proj1", str(project_dir),
        FakeGenerator(), FakeVerifier(), logs_dir=str(logs_dir),
    )
    assert result == {"skipped": True}


def test_incremental_does_not_rejudge_same_hash(tmp_path):
    notes_dir, _ = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    projects = {"proj1": str(project_dir)}
    gen = FakeGenerator({"score": 0.9})
    ver = FakeVerifier()
    logs = str(tmp_path / "logs")
    first = run_incremental(str(notes_dir), projects, gen, ver, logs_dir=logs)
    assert first["judged"] == 1
    second = run_incremental(str(notes_dir), projects, gen, ver, logs_dir=logs)
    assert second["judged"] == 0
    assert second["skipped_pairs"] == 1


def test_rejudges_changed_hash(tmp_path):
    notes_dir, note_file = make_note(tmp_path, content="original")
    project_dir = make_project(tmp_path)
    projects = {"proj1": str(project_dir)}
    gen = FakeGenerator({"score": 0.9})
    ver = FakeVerifier()
    logs = str(tmp_path / "logs")
    run_incremental(str(notes_dir), projects, gen, ver, logs_dir=logs)
    note_file.write_text("changed content", encoding="utf-8")
    result = run_incremental(str(notes_dir), projects, gen, ver, logs_dir=logs)
    assert result["judged"] == 1


def test_adjudications_json_structure(tmp_path):
    notes_dir, _ = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    logs = tmp_path / "logs"
    run_incremental(str(notes_dir), {"proj1": str(project_dir)},
                    FakeGenerator({"score": 0.9}), FakeVerifier(), logs_dir=str(logs))
    state_file = logs / "adjudications.json"
    assert state_file.exists()
    data = json.loads(state_file.read_text(encoding="utf-8"))
    assert "pairs" in data and "by_project" in data
    for entries in data["by_project"].values():
        for entry in entries:
            assert entry["strong"] is True


def test_by_project_no_duplicates_across_runs(tmp_path):
    notes_dir, _ = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    projects = {"proj1": str(project_dir)}
    gen = FakeGenerator({"score": 0.9})
    ver = FakeVerifier()
    logs = tmp_path / "logs"
    run_incremental(str(notes_dir), projects, gen, ver, logs_dir=str(logs))
    run_incremental(str(notes_dir), projects, gen, ver, logs_dir=str(logs))
    data = json.loads((logs / "adjudications.json").read_text(encoding="utf-8"))
    assert len(data["by_project"]["proj1"]) == 1


def test_applies_to_in_frontmatter_after_strong_match(tmp_path):
    notes_dir, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    run_incremental(str(notes_dir), {"proj1": str(project_dir)},
                    FakeGenerator({"score": 0.9}), FakeVerifier(),
                    logs_dir=str(tmp_path / "logs"))
    content = note_file.read_text(encoding="utf-8")
    assert "applies_to" in content
    assert "proj1" in content
    assert "title: nota" in content  # frontmatter previo preservado
    assert "cuerpo" in content       # cuerpo preservado


def test_codegraph_present_when_dir_exists(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    codegraph_dir = project_dir / ".codegraph"
    codegraph_dir.mkdir()
    (project_dir / "modulo.py").write_text("print('hola')", encoding="utf-8")
    gen = CapturingGenerator()
    adjudicate(note_dict(note_file), "proj1", str(project_dir), gen, FakeVerifier(),
               logs_dir=str(tmp_path / "logs"))
    assert isinstance(gen.last_payload["codegraph"], list)


def test_codegraph_none_when_dir_missing(tmp_path):
    _, note_file = make_note(tmp_path)
    project_dir = make_project(tmp_path)
    gen = CapturingGenerator()
    adjudicate(note_dict(note_file), "proj1", str(project_dir), gen, FakeVerifier(),
               logs_dir=str(tmp_path / "logs"))
    assert gen.last_payload["codegraph"] is None
