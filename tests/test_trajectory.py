"""trajectory: captura+compresion de loops -> dataset flywheel + skill distill.
Idea robada de Hermes (trajectory compression + autonomous skill creation)."""
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
RL = importlib.import_module("mmorch.rubric_loop")
TR = importlib.import_module("mmorch.trajectory")

GOOD = "```python\ndef inc(x):\n    return x + 1\n```"
BAD = "```python\ndef inc(x):\n    return x - 1\n```"
CHECKABLE = [{"id": "c1", "desc": "inc pasa", "kind": "checkable", "checker": "python_exec",
              "ctx": {"code": "{attempt_code}\nassert inc(1)==2\nassert inc(-1)==0"}}]


def test_compress_captures_steps_and_exec_labels(tmp_path, monkeypatch):
    monkeypatch.setattr(RL, "_close_loop", lambda s: None)   # no escribir todavia
    attempts = iter([BAD, GOOD])
    st = RL.run_rubric_loop("inc", CHECKABLE, gen_fn=lambda p: next(attempts))
    traj = TR.compress(st)
    assert traj["passed"] and traj["n_iters"] == 2 and len(traj["steps"]) == 2
    # paso 1 fallo la ejecucion, paso 2 paso -> labels de EJECUCION correctos
    assert traj["steps"][0]["checkable_pass"] is False
    assert traj["steps"][1]["checkable_pass"] is True


def test_record_and_dataset_roundtrip(tmp_path, monkeypatch):
    p = tmp_path / "traj.jsonl"
    monkeypatch.setattr(TR, "_TRAJ", p)
    monkeypatch.setattr(TR, "_SKILLS", tmp_path / "skills.jsonl")
    monkeypatch.setattr("mmorch.memory.write_note", lambda *a, **k: 0)
    monkeypatch.setattr(RL, "_close_loop", lambda s: TR.record_trajectory(s, path=p))
    attempts = iter([BAD, GOOD])
    RL.run_rubric_loop("inc", CHECKABLE, gen_fn=lambda p: next(attempts))
    ds = TR.trajectory_dataset(p)
    # 2 codigos distintos (BAD label 0, GOOD label 1) = training data ejecucion-etiquetada
    labels = sorted(d["label"] for d in ds)
    assert labels == [0, 1]
    assert all(d["source"] == "trajectory" for d in ds)


def test_skill_distilled_only_after_corrections(tmp_path, monkeypatch):
    sp = tmp_path / "skills.jsonl"
    monkeypatch.setattr(TR, "_TRAJ", tmp_path / "t.jsonl")
    monkeypatch.setattr(TR, "_SKILLS", sp)
    notes = []
    monkeypatch.setattr("mmorch.memory.write_note", lambda scope, text, **k: notes.append(text) or 0)
    monkeypatch.setattr(RL, "_close_loop", lambda s: TR.record_trajectory(s, path=TR._TRAJ))
    # verde en 1 intento -> NO skill (no hubo correccion que aprender)
    RL.run_rubric_loop("inc", CHECKABLE, gen_fn=lambda p: GOOD)
    assert not sp.exists()
    # verde tras 2 intentos -> SI skill
    attempts = iter([BAD, GOOD])
    RL.run_rubric_loop("inc", CHECKABLE, gen_fn=lambda p: next(attempts))
    skills = [json.loads(l) for l in sp.read_text(encoding="utf-8").splitlines()]
    assert len(skills) == 1 and skills[0]["fixed_criteria"] == ["c1"]
    assert "def inc(x):" in skills[0]["winning_code"]
    assert notes   # nota verificada escrita


def test_skill_not_distilled_without_execution_verification(tmp_path):
    # trayectoria verde-tras-correccion pero SOLO juez subjetivo (sin checkable) -> NO destila
    # (anti-degradacion: memoria verified solo con verdad de EJECUCION, no opinion)
    sp = tmp_path / "skills.jsonl"
    traj = {"task": "x", "criteria": [{"id": "s1", "desc": "lindo", "kind": "subjective"}],
            "steps": [{"iter": 1, "code": "def f(): pass", "failed": ["s1"], "checkable_pass": False},
                      {"iter": 2, "code": "def f(): return 1", "failed": [], "checkable_pass": False}],
            "n_iters": 2, "reward": 1.0, "passed": True}
    assert TR.distill_skill(traj, path=sp) == {}
    assert not sp.exists()


def test_dataset_skips_trajectories_without_checkable(tmp_path):
    p = tmp_path / "t.jsonl"
    p.write_text(json.dumps({
        "task": "x", "criteria": [{"id": "s1", "desc": "lindo", "kind": "subjective"}],
        "steps": [{"iter": 1, "code": "def f(): pass", "failed": [], "checkable_pass": False}],
        "n_iters": 1, "reward": 1.0, "passed": True}) + "\n", encoding="utf-8")
    assert TR.trajectory_dataset(p) == []   # sin checker => sin label confiable => fuera
