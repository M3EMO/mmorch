"""workflow_engine — cooperative multi-role workflow as a pure state machine (Phase C).

The ChatDev *dynamics* (architect -> coder -> reviewer hand-off, review loop) ported to mmorch's
edge: cero-cupo, truth=execution (test gate) and cross-family verdicts (not self-assessment),
durable block-context checkpoints (Phase A) + resume (Phase B).

Design = the rubric_loop pattern generalized to N roles. PURE state machine (no providers/store
imports): start_workflow -> next_workflow_action -> submit_workflow. The DRIVER (server) executes
each action and feeds the result back. JSON-serializable state -> Phase B resume works unchanged.

A step: {role, model, consumes:[produce-names], produces:name, gate:"none"|"tests"|"verdict",
         test_cmd?, loop_back:int?(step idx), max:int}.
Two phases per step: "produce" (run the role's model) then optional "gate":
  - tests   -> the driver runs test_cmd; truth = execution.
  - verdict -> the step IS a cross-family reviewer (its model, cross-family vs the producer of what
               it consumes — validated at load); the driver parses approve/refute from its output.
Block flow: a step's output block derives_from the blocks it consumed (lineage = ancestry-everywhere).
On gate fail: loop back to the configured step up to `max`, else escalate.
"""
from __future__ import annotations

_TERMINAL = ("done", "escalate")
_GATES = ("none", "tests", "verdict")


def start_workflow(steps: list, task: str) -> dict:
    return {"task": task, "steps": steps, "cursor": 0, "produced": {}, "history": [],
            "loops": {}, "status": "running", "phase": "produce", "pending": None}


def _step(state: dict) -> dict:
    return state["steps"][state["cursor"]]


def next_workflow_action(state: dict) -> dict:
    """What the driver must do now: produce | gate | done | escalate (pure, no side effects)."""
    if state["status"] != "running":
        return {"kind": state["status"], "produced": state["produced"]}
    st = _step(state)
    if state["phase"] == "produce":
        consumes = [state["produced"][c] for c in st.get("consumes", []) if c in state["produced"]]
        return {"kind": "produce", "step": state["cursor"], "role": st["role"],
                "model": st.get("model"), "persona": st.get("persona", ""), "gate": st.get("gate", "none"),
                "consumes": consumes, "produces": st.get("produces"), "task": state["task"]}
    # phase == gate
    return {"kind": "gate", "step": state["cursor"], "gate": st.get("gate"),
            "block": state["pending"], "test_cmd": st.get("test_cmd"), "role": st["role"]}


def submit_workflow(state: dict, *, block_id: str | None = None, gate_passed: bool | None = None) -> dict:
    """Feed a result back. produce -> pass block_id; gate -> pass gate_passed. Mutates + returns."""
    if state["status"] != "running":
        return state
    st = _step(state)
    if state["phase"] == "produce":
        state["produced"][st.get("produces")] = block_id
        state["pending"] = block_id
        state["history"].append({"step": state["cursor"], "role": st["role"],
                                 "block": block_id, "event": "produced"})
        if st.get("gate", "none") in ("tests", "verdict"):
            state["phase"] = "gate"
        else:
            _advance(state)
        return state
    # phase == gate
    state["history"].append({"step": state["cursor"], "event": "gate",
                             "gate": st.get("gate"), "passed": bool(gate_passed)})
    if gate_passed:
        _advance(state)
    else:
        _on_fail(state)
    return state


def _advance(state: dict) -> None:
    state["cursor"] += 1
    state["phase"] = "produce"
    state["pending"] = None
    if state["cursor"] >= len(state["steps"]):
        state["status"] = "done"


def _on_fail(state: dict) -> None:
    """Gate failed: loop back to the configured step (up to max), else escalate."""
    st = _step(state)
    lb = st.get("loop_back")
    key = str(state["cursor"])
    cnt = state["loops"].get(key, 0)
    if lb is not None and cnt < int(st.get("max", 3)):
        state["loops"][key] = cnt + 1
        state["cursor"] = int(lb)
        state["phase"] = "produce"
        state["pending"] = None
    else:
        state["status"] = "escalate"


def build_prompt(role: str, persona: str, task: str, inputs: list) -> str:
    """Inception prompt for a role step. inputs = [(produce_name, block_body), ...] from prior roles."""
    parts = [persona.strip()] if persona else []
    parts.append(f"TASK:\n{task}")
    if inputs:
        parts.append("INPUTS FROM PRIOR ROLES:\n" + "\n\n".join(
            f"--- {name} ---\n{body}" for name, body in inputs))
    parts.append(f"You are the {role}. Produce ONLY your artifact (code in a ``` block when code).")
    return "\n\n".join(parts)


if __name__ == "__main__":
    steps = [
        {"role": "architect", "produces": "plan", "gate": "none"},
        {"role": "coder", "consumes": ["plan"], "produces": "code", "gate": "tests", "test_cmd": "pytest"},
        {"role": "reviewer", "consumes": ["code"], "produces": "review", "gate": "verdict",
         "loop_back": 1, "max": 2},
    ]

    # --- happy path with ONE review-loop bounce ---
    st = start_workflow(steps, "build X")
    a = next_workflow_action(st)
    assert a["kind"] == "produce" and a["role"] == "architect" and a["consumes"] == []
    submit_workflow(st, block_id="PLAN1")                      # gate none -> advance
    a = next_workflow_action(st)
    assert a["role"] == "coder" and a["consumes"] == ["PLAN1"], a   # consumes plan's block (lineage)
    submit_workflow(st, block_id="CODE1")                      # gate tests -> phase gate
    g = next_workflow_action(st)
    assert g["kind"] == "gate" and g["gate"] == "tests" and g["test_cmd"] == "pytest"
    submit_workflow(st, gate_passed=True)                      # tests pass -> reviewer
    a = next_workflow_action(st)
    assert a["role"] == "reviewer" and a["consumes"] == ["CODE1"]
    submit_workflow(st, block_id="REV1")                       # gate verdict
    assert next_workflow_action(st)["gate"] == "verdict"
    submit_workflow(st, gate_passed=False)                     # verdict FAIL -> loop back to coder
    assert st["cursor"] == 1 and st["loops"]["2"] == 1, st
    submit_workflow(st, block_id="CODE2"); submit_workflow(st, gate_passed=True)   # coder re-run, tests pass
    submit_workflow(st, block_id="REV2"); submit_workflow(st, gate_passed=True)    # reviewer pass -> done
    assert st["status"] == "done" and st["produced"]["code"] == "CODE2"
    assert next_workflow_action(st)["kind"] == "done"

    # --- escalate: tests pass, verdict always fails -> max loop-backs then escalate ---
    st2 = start_workflow(steps, "build Y")
    for _ in range(20):
        act = next_workflow_action(st2)
        if act["kind"] in _TERMINAL:
            break
        if act["kind"] == "produce":
            submit_workflow(st2, block_id="x")
        elif act["gate"] == "tests":
            submit_workflow(st2, gate_passed=True)
        else:
            submit_workflow(st2, gate_passed=False)
    assert st2["status"] == "escalate" and st2["loops"]["2"] == 2, st2

    # --- tests-gate fail with NO loop_back -> escalate ---
    st3 = start_workflow([{"role": "coder", "produces": "code", "gate": "tests", "test_cmd": "pytest"}], "z")
    submit_workflow(st3, block_id="C"); submit_workflow(st3, gate_passed=False)
    assert st3["status"] == "escalate"

    # --- build_prompt ---
    p = build_prompt("coder", "You are a coder.", "do X", [("plan", "PLAN BODY")])
    assert "You are the coder" in p and "PLAN BODY" in p and "do X" in p
    print("workflow_engine OK")
