import textwrap

import pytest

from mmorch import bughunt


@pytest.fixture
def repo_dir(tmp_path):
    """Create a minimal repo structure with one module and its test."""
    mod_dir = tmp_path / "mmorch"
    test_dir = tmp_path / "tests"
    mod_dir.mkdir()
    test_dir.mkdir()

    (mod_dir / "sample.py").write_text(
        textwrap.dedent(
            """\
            def check(x):
                if x > 10:
                    return "big"
                return "small"
            """
        )
    )
    (test_dir / "test_sample.py").write_text("def test_dummy(): pass\n")
    return tmp_path


def test_module_pairs_finds_pairs_and_omits_without_tests(repo_dir):
    # Add a module without a test
    (repo_dir / "mmorch" / "untested.py").write_text("x = 1\n")

    pairs = bughunt.module_pairs(str(repo_dir))

    assert pairs == [("mmorch/sample.py", "tests/test_sample.py")]


def test_survivors_for_restores_original_always(repo_dir):
    original = (repo_dir / "mmorch" / "sample.py").read_text()

    def run_fn(test_rel):
        return True

    result = bughunt.survivors_for(
        "mmorch/sample.py",
        "tests/test_sample.py",
        repo_dir=str(repo_dir),
        run_fn=run_fn,
    )

    assert (repo_dir / "mmorch" / "sample.py").read_text() == original
    assert result["survived"] > 0


def test_survivors_for_restores_even_if_run_fn_raises(repo_dir):
    original = (repo_dir / "mmorch" / "sample.py").read_text()

    def run_fn(test_rel):
        raise RuntimeError("boom")

    with pytest.raises(RuntimeError):
        bughunt.survivors_for(
            "mmorch/sample.py",
            "tests/test_sample.py",
            repo_dir=str(repo_dir),
            run_fn=run_fn,
        )

    assert (repo_dir / "mmorch" / "sample.py").read_text() == original


def test_dead_mutant_not_in_survivor_diffs(repo_dir):
    def run_fn(test_rel):
        return False

    result = bughunt.survivors_for(
        "mmorch/sample.py",
        "tests/test_sample.py",
        repo_dir=str(repo_dir),
        run_fn=run_fn,
    )

    assert result["survivor_diffs"] == []


def test_live_mutant_appears_with_non_empty_diff(repo_dir):
    def run_fn(test_rel):
        return True

    result = bughunt.survivors_for(
        "mmorch/sample.py",
        "tests/test_sample.py",
        repo_dir=str(repo_dir),
        run_fn=run_fn,
    )

    assert result["survived"] > 0
    assert all(diff for diff in result["survivor_diffs"])


def test_precondition_red_base_skips_without_touching_file(repo_dir):
    original = (repo_dir / "mmorch" / "sample.py").read_text()

    def run_fn(test_rel):
        return False

    result = bughunt.survivors_for(
        "mmorch/sample.py",
        "tests/test_sample.py",
        repo_dir=str(repo_dir),
        run_fn=run_fn,
    )

    assert result == {"module": "mmorch/sample.py", "skipped": "suite roja de base"}
    assert (repo_dir / "mmorch" / "sample.py").read_text() == original


def test_hunt_adds_findings_only_with_review_fn_and_survivors(repo_dir):
    def run_fn(test_rel):
        return True

    def review_fn(module_rel, survivor_diffs):
        return ["posible bug en comparacion"]

    result = bughunt.hunt(
        str(repo_dir),
        run_fn=run_fn,
        review_fn=review_fn,
    )

    assert result["scanned"] == 1
    assert len(result["findings"]) == 1
    assert result["findings"][0]["module"] == "mmorch/sample.py"
    assert result["findings"][0]["findings"] == ["posible bug en comparacion"]

    # Without review_fn, no findings
    result_no_review = bughunt.hunt(str(repo_dir), run_fn=run_fn)
    assert result_no_review["findings"] == []


def test_hunt_fail_soft_survivors_for_raises_goes_to_errors(repo_dir):
    def run_fn(test_rel):
        raise RuntimeError("boom")

    result = bughunt.hunt(str(repo_dir), run_fn=run_fn)

    assert result["scanned"] == 1
    assert len(result["errors"]) == 1
    assert "mmorch/sample.py" in result["errors"][0]
