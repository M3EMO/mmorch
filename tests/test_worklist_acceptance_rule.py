"""validate_worklist rechaza una unidad que ES el test de aceptacion (A/B chatbot 2026-09-10).

El planner habia creado 'acceptance-test' con test_cmd = external_test: esa unidad no puede
pasar sola, su gate nunca dio verde, el engine escalo y la integracion nunca corrio. $0."""
from mmorch.project_build import validate_worklist

EXT = 'cd backend && "C:\\tools\\mvn.cmd" -q -B test'


def _units(extra=None):
    base = [
        {"name": "msg", "spec": "Msg class", "file": "b/Msg.java", "deps": [], "test_cmd": None},
        {"name": "bot", "spec": "Bot class", "file": "b/Bot.java", "deps": ["msg"], "test_cmd": None},
    ]
    return base + ([extra] if extra else [])


def test_plain_plan_passes():
    ok, errs = validate_worklist(_units(), EXT)
    assert ok, errs


def test_acceptance_unit_by_name_rejected():
    u = {"name": "acceptance-test", "spec": "run it", "file": "b/T.java", "deps": ["bot"], "test_cmd": None}
    ok, errs = validate_worklist(_units(u), EXT)
    assert not ok and any("INTEGRATION gate" in e for e in errs)


def test_unit_whose_test_cmd_is_the_external_test_rejected():
    u = {"name": "final", "spec": "wire", "file": "b/F.java", "deps": ["bot"], "test_cmd": "mvn -q -B test"}
    ok, errs = validate_worklist(_units(u), EXT)
    assert not ok and any("final" in e for e in errs)


def test_no_external_test_keeps_old_behaviour():
    u = {"name": "final", "spec": "wire", "file": "b/F.java", "deps": ["bot"], "test_cmd": "pytest -q"}
    ok, errs = validate_worklist(_units(u))
    assert ok, errs
