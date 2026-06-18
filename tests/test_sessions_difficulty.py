import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")
Seg = S.Segment


def test_one_tool_call_is_clear():
    assert S.observed_domain(Seg(request="x", tool_calls=[{"name": "Read", "input": {}}])) == "clear"


def test_few_tool_calls_complicated():
    tc = [{"name": "Edit", "input": {}} for _ in range(3)]
    assert S.observed_domain(Seg(request="x", tool_calls=tc)) == "complicated"


def test_many_tool_calls_complex():
    tc = [{"name": "Edit", "input": {}} for _ in range(8)]
    assert S.observed_domain(Seg(request="x", tool_calls=tc)) == "complex"


def test_revert_is_chaotic():
    seg = Seg(request="x", tool_calls=[{"name": "Bash", "input": {"command": "git revert HEAD"}}])
    assert S.observed_domain(seg) == "chaotic"


def test_error_result_is_chaotic():
    seg = Seg(request="x", tool_calls=[{"name": "Bash", "input": {}}],
              tool_results=[{"content": "boom", "is_error": True}])
    assert S.observed_domain(seg) == "chaotic"
