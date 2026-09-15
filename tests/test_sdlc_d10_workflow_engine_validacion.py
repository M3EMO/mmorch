"""D10 (robustez por modulos: `workflow_engine`): la maquina de estados valida sus pasos al cargar.

El docstring promete "validated at load" pero `_GATES` no se usa: hoy un workflow malformado
explota mas tarde con IndexError/KeyError dentro del driver. Requisitos, puros y sin API:
- R1 `start_workflow([], task)` levanta ValueError que menciona "steps".
- R2 un step con `gate` fuera de ("none", "tests", "verdict") levanta ValueError que menciona "gate".
- R3 un step con `loop_back` fuera de rango (>= indice del step o negativo) levanta ValueError que menciona "loop_back".
- R4 un step sin `role` levanta ValueError que menciona "role".
- R5 `submit_workflow` en fase produce sin `block_id` levanta ValueError que menciona "block_id".
- R6 un workflow valido sigue funcionando igual (camino feliz de una vuelta).
"""
import pytest

from mmorch.workflow_engine import next_workflow_action, start_workflow, submit_workflow


def _ok_steps():
    return [{"role": "architect", "produces": "plan", "gate": "none"},
            {"role": "coder", "consumes": ["plan"], "produces": "code", "gate": "tests", "test_cmd": "pytest",
             "loop_back": 0, "max": 1}]


def test_R1_steps_vacios():
    with pytest.raises(ValueError, match="steps"):
        start_workflow([], "t")


def test_R2_gate_desconocido():
    with pytest.raises(ValueError, match="gate"):
        start_workflow([{"role": "coder", "produces": "code", "gate": "vibes"}], "t")


def test_R3_loop_back_fuera_de_rango():
    with pytest.raises(ValueError, match="loop_back"):
        start_workflow([{"role": "coder", "produces": "code", "gate": "tests", "test_cmd": "x", "loop_back": 5}], "t")
    with pytest.raises(ValueError, match="loop_back"):
        start_workflow([{"role": "coder", "produces": "code", "gate": "tests", "test_cmd": "x", "loop_back": -1}], "t")


def test_R4_step_sin_role():
    with pytest.raises(ValueError, match="role"):
        start_workflow([{"produces": "code", "gate": "none"}], "t")


def test_R5_submit_produce_sin_block_id():
    st = start_workflow(_ok_steps(), "t")
    with pytest.raises(ValueError, match="block_id"):
        submit_workflow(st)


def test_R6_camino_feliz_intacto():
    st = start_workflow(_ok_steps(), "t")
    assert next_workflow_action(st)["role"] == "architect"
    submit_workflow(st, block_id="P1")
    assert next_workflow_action(st)["consumes"] == ["P1"]
    submit_workflow(st, block_id="C1")
    assert next_workflow_action(st)["kind"] == "gate"
    submit_workflow(st, gate_passed=True)
    assert st["status"] == "done" and st["produced"]["code"] == "C1"
