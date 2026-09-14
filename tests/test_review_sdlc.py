"""Review del diff D10: validate_steps() en mmorch/workflow_engine.py.

R3 dice: si el step tiene la clave "loop_back", y loop_back < 0 o loop_back >= i,
se levanta ValueError con "loop_back" en el mensaje. Un step con
`"loop_back": None` TIENE la clave (no esta ausente), asi que R3 debe aplicar
y levantar ValueError. La implementacion hace `lb < 0` sobre `lb = None` sin
chequear el tipo antes, y eso levanta TypeError en vez de ValueError.
"""
import pytest

from mmorch.workflow_engine import start_workflow


def test_r3_loop_back_none_levanta_valueerror_no_typeerror():
    with pytest.raises(ValueError, match="loop_back"):
        start_workflow(
            [{"role": "coder", "produces": "code", "gate": "tests",
              "test_cmd": "x", "loop_back": None}],
            "t",
        )
