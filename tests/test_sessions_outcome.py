import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")
Seg = S.Segment


def test_tests_passed_is_positive():
    seg = Seg(request="corre tests", tool_results=[{"content": "5 passed in 2s", "is_error": False}])
    o = S.outcome_of(seg)
    assert o is not None and o.reward >= 0.8 and o.source == "tool"


def test_user_acceptance_is_positive():
    seg = Seg(request="hace X")
    o = S.outcome_of(seg, next_request="perfecto, funciona")
    assert o is not None and o.reward >= 0.8 and o.source == "user"


def test_user_rejection_is_negative():
    seg = Seg(request="hace X")
    o = S.outcome_of(seg, next_request="no, esta mal, rehacelo")
    assert o is not None and o.reward <= 0.2


def test_tests_failed_is_negative():
    seg = Seg(request="corre tests", tool_results=[{"content": "1 failed, 4 passed", "is_error": False}])
    o = S.outcome_of(seg)
    assert o is not None and o.reward <= 0.2


def test_no_signal_abstains():
    seg = Seg(request="contame algo", reasoning="bla")
    assert S.outcome_of(seg) is None


def test_claude_self_report_is_not_a_label():
    seg = Seg(request="hace X", reasoning="listo, funciona perfecto, done")
    assert S.outcome_of(seg) is None


def test_incidental_error_word_is_not_a_failure():
    # precision fix (mmorch verify T2): "error" suelto en un log exitoso no marca fallo.
    seg = Seg(request="x", tool_results=[{"content": "5 passed; tested error handling path", "is_error": False}])
    assert S.outcome_of(seg).reward >= 0.8


def test_zero_failed_is_a_pass():
    seg = Seg(request="x", tool_results=[{"content": "5 passed, 0 failed", "is_error": False}])
    assert S.outcome_of(seg).reward >= 0.8
