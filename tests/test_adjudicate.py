import json
import os
import sys

# Ensure the repo root is on sys.path so mmorch is importable
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Mock dotenv and yaml before importing mmorch to avoid dependencies
sys.modules['dotenv'] = type(sys)('dotenv')
sys.modules['dotenv'].load_dotenv = lambda *args, **kwargs: None
sys.modules['yaml'] = type(sys)('yaml')
sys.modules['yaml'].safe_load = lambda *args, **kwargs: {}
sys.modules['yaml'].safe_dump = lambda *args, **kwargs: ''

from mmorch.adjudicate import (
    adjudicate,
    run_incremental,
    build_payload,
)


class FakeGenerator:
    def __init__(self, response=None):
        self.response = response or {"strong": False, "refuted": False, "score": 0.0}

    def propose(self, payload):
        return self.response


class FakeVerifier:
    def __init__(self, response=None):
        self.response = response or {"refuted": False, "score": 0.0}

    def refute(self, payload):
        return self.response


def make_notes(tmp_path, content="note content"):
    notes_dir = tmp_path / "notes"
    notes_dir.mkdir(exist_ok=True)
    note_file = notes_dir / "note1.md"
    note_file.write_text(content)
    return notes_dir


def make_projects(tmp_path):
    projects_dir = tmp_path / "projects"
    projects_dir.mkdir(exist_ok=True)
    project_dir = projects_dir / "proj1"
    project_dir.mkdir(exist_ok=True)
    return projects_dir


def test_strong_match_survives_refutation(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": False, "score": 0.9})
    verifier = FakeVerifier({"refuted": False, "score": 0.9})
    result = adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    assert result["strong"] is True


def test_low_score_not_strong(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": False, "refuted": False, "score": 0.3})
    verifier = FakeVerifier({"refuted": False, "score": 0.3})
    result = adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    assert result["strong"] is False


def test_refuted_not_strong(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": True, "score": 0.9})
    verifier = FakeVerifier({"refuted": True, "score": 0.9})
    result = adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    assert result["strong"] is False


def test_loop_paused(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    logs_dir = tmp_path / "logs"
    logs_dir.mkdir(exist_ok=True)
    (logs_dir / "loop_paused").touch()
    generator = FakeGenerator()
    verifier = FakeVerifier()
    result = adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
        logs_dir=logs_dir,
    )
    assert result == {"skipped": True}


def test_incremental_does_not_rejudge_same_hash(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": False, "score": 0.9})
    verifier = FakeVerifier({"refuted": False, "score": 0.9})
    run_incremental(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    result = run_incremental(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    assert result["judged"] == 0


def test_rejudges_changed_hash(tmp_path):
    notes_dir = make_notes(tmp_path, content="original")
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": False, "score": 0.9})
    verifier = FakeVerifier({"refuted": False, "score": 0.9})
    run_incremental(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    # modify note content
    note_file = notes_dir / "note1.md"
    note_file.write_text("changed content")
    result = run_incremental(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    assert result["judged"] > 0


def test_adjudications_json_written_atomically(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": False, "score": 0.9})
    verifier = FakeVerifier({"refuted": False, "score": 0.9})
    adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    adjudications_file = tmp_path / "adjudications.json"
    assert adjudications_file.exists()
    data = json.loads(adjudications_file.read_text())
    assert "pairs" in data
    assert "by_project" in data
    # by_project only strong
    for _project, entries in data["by_project"].items():
        for entry in entries:
            assert entry["strong"] is True


def test_applies_to_in_frontmatter_after_strong_match(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    generator = FakeGenerator({"strong": True, "refuted": False, "score": 0.9})
    verifier = FakeVerifier({"refuted": False, "score": 0.9})
    adjudicate(
        notes_dir=notes_dir,
        projects_dir=projects_dir,
        generator=generator,
        verifier=verifier,
    )
    note_file = notes_dir / "note1.md"
    content = note_file.read_text()
    assert "applies_to" in content


def test_codegraph_present_when_dir_exists(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    project_dir = projects_dir / "proj1"
    codegraph_dir = project_dir / ".codegraph"
    codegraph_dir.mkdir(exist_ok=True)
    (codegraph_dir / "file.py").write_text("print('hello')")
    payload = build_payload(
        note_path=notes_dir / "note1.md",
        project_path=project_dir,
    )
    assert isinstance(payload["codegraph"], list)


def test_codegraph_none_when_dir_missing(tmp_path):
    notes_dir = make_notes(tmp_path)
    projects_dir = make_projects(tmp_path)
    project_dir = projects_dir / "proj1"
    payload = build_payload(
        note_path=notes_dir / "note1.md",
        project_path=project_dir,
    )
    assert payload["codegraph"] is None
