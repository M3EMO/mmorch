"""workflow_spec — load + validate cooperative workflows and role personas (Phase C, Decisions #2/#3).

Policy-as-data, same mold as plugins/budget:
- roles:     roles/<name>.md       (plain-text persona; hand-editable). load_role(name).
- workflows: workflows/<name>.workflow.json. load_workflow(name) / discover_workflows().

A workflow JSON: {name, description?, steps:[{role, model?, consumes?, produces?, gate?, test_cmd?,
loop_back?(role name), max?}]}. load_workflow normalizes (inject persona from the role registry,
defaults, resolve loop_back role->step index) and VALIDATES: gate enum, consumes produced upstream,
loop_back resolvable, and the OneFlow rule for verdict steps (reviewer model cross-family vs the
producer of what it reviews). Dirs overridable via MMORCH_ROLES_DIR / MMORCH_WORKFLOWS_DIR.
"""
from __future__ import annotations

import json
import os
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
_GATES = ("none", "tests", "verdict")


def roles_dir() -> Path:
    return Path(os.getenv("MMORCH_ROLES_DIR") or (_ROOT / "roles"))


def workflows_dir() -> Path:
    return Path(os.getenv("MMORCH_WORKFLOWS_DIR") or (_ROOT / "workflows"))


def load_role(name: str) -> str:
    """Persona text for a role. Missing file -> a minimal generic persona (never blocks a run)."""
    f = roles_dir() / f"{name}.md"
    if f.is_file():
        return f.read_text(encoding="utf-8").strip()
    return f"You are the {name}. Do your part of the task precisely and concisely."


def discover_workflows() -> list:
    base = workflows_dir()
    out = []
    if not base.is_dir():
        return out
    for f in sorted(base.glob("*.workflow.json")):
        try:
            spec = json.loads(f.read_text(encoding="utf-8"))
            out.append({"name": spec.get("name", f.stem.replace(".workflow", "")),
                        "description": spec.get("description", ""), "steps": len(spec.get("steps", []))})
        except Exception as e:
            out.append({"name": f.stem, "error": str(e)[:160]})
    return out


def _family_of(model: str):
    try:
        from .config import family_of
        return family_of(model)
    except Exception:
        return None        # registry unavailable / unknown model -> can't prove cross-family


def validate(spec: dict) -> dict:
    """Normalize + validate. Returns the normalized spec (steps with persona/defaults/resolved
    loop_back). Raises ValueError on a real config error."""
    steps = spec.get("steps")
    if not isinstance(steps, list) or not steps:
        raise ValueError("workflow needs a non-empty 'steps' list")
    produced_by = {}                                  # produce-name -> step index
    role_to_index = {}
    norm = []
    for i, raw in enumerate(steps):
        role = raw.get("role")
        if not role:
            raise ValueError(f"step {i}: missing 'role'")
        gate = raw.get("gate", "none")
        if gate not in _GATES:
            raise ValueError(f"step {i} ({role}): bad gate '{gate}' (one of {_GATES})")
        produces = raw.get("produces") or role
        consumes = list(raw.get("consumes", []))
        for c in consumes:
            if c not in produced_by:
                raise ValueError(f"step {i} ({role}): consumes '{c}' not produced by an earlier step")
        if gate == "tests" and not raw.get("test_cmd"):
            raise ValueError(f"step {i} ({role}): gate 'tests' requires 'test_cmd'")
        step = {"role": role, "model": raw.get("model"), "consumes": consumes,
                "produces": produces, "gate": gate, "test_cmd": raw.get("test_cmd"),
                "max": int(raw.get("max", 3)), "persona": load_role(role)}
        # loop_back: a role NAME -> resolve to that role's (earliest) step index
        lb = raw.get("loop_back")
        if lb is not None:
            if lb not in role_to_index:
                raise ValueError(f"step {i} ({role}): loop_back '{lb}' is not an earlier role")
            step["loop_back"] = role_to_index[lb]
        norm.append(step)
        produced_by[produces] = i
        role_to_index.setdefault(role, i)
    # OneFlow: a verdict step's model must be cross-family vs the producer of what it consumes.
    for i, step in enumerate(norm):
        if step["gate"] != "verdict":
            continue
        if not step["model"]:
            raise ValueError(f"step {i} ({step['role']}): verdict gate requires an explicit 'model'")
        for c in step["consumes"]:
            prod = norm[produced_by[c]]
            fa, fb = _family_of(step["model"]), _family_of(prod.get("model") or "")
            if fa and fb and fa == fb:
                raise ValueError(
                    f"step {i} ({step['role']}): OneFlow — verdict model '{step['model']}' must be "
                    f"cross-family vs the '{c}' producer '{prod.get('model')}' (both {fa})")
    return {"name": spec.get("name", "workflow"), "description": spec.get("description", ""),
            "steps": norm}


def load_workflow(name: str) -> dict:
    f = workflows_dir() / (name if name.endswith(".workflow.json") else f"{name}.workflow.json")
    if not f.is_file():
        raise FileNotFoundError(f"no workflow '{name}' in {workflows_dir()}")
    return validate(json.loads(f.read_text(encoding="utf-8")))


if __name__ == "__main__":
    # validate the committed example + the failure modes (run via venv: cross-family needs the registry)
    wf = load_workflow("build-feature")
    roles = [s["role"] for s in wf["steps"]]
    assert roles == ["architect", "coder", "reviewer"], roles
    coder = wf["steps"][1]
    assert coder["gate"] == "tests" and coder["test_cmd"], coder
    rev = wf["steps"][2]
    assert rev["gate"] == "verdict" and rev["loop_back"] == 1, rev          # loop_back coder -> idx 1
    assert wf["steps"][0]["persona"] and "architect" in wf["steps"][0]["persona"].lower()

    def bad(spec, frag):
        try:
            validate(spec); assert False, f"expected ValueError for {frag}"
        except ValueError as e:
            assert frag in str(e), (frag, str(e))

    bad({"steps": [{"role": "x", "consumes": ["nope"]}]}, "not produced")
    bad({"steps": [{"role": "x", "gate": "bogus"}]}, "bad gate")
    bad({"steps": [{"role": "x", "gate": "tests"}]}, "requires 'test_cmd'")
    bad({"steps": [{"role": "a", "produces": "p", "model": "deepseek-chat"},
                   {"role": "b", "consumes": ["p"], "gate": "verdict", "model": "deepseek-chat",
                    "loop_back": "a"}]}, "cross-family")
    assert discover_workflows() and any(w["name"] == "build-feature" for w in discover_workflows())
    print("workflow_spec OK")
