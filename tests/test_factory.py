"""tests factory.py — mmorch entrena modelos chicos (CPU) + emite jobs grandes. Sin API."""
from mmorch.factory import (featurize_code, train_logreg, train_code_quality,
                            emit_training_job, accuracy)


def test_featurize_shape_and_signals():
    f = featurize_code("def f():\n    \"\"\"doc\"\"\"\n    return 1\n")
    assert len(f) == 9
    assert f[2] == 1.0           # ast_ok
    assert f[3] == 1.0           # has_def
    bad = featurize_code("def f(:\n")
    assert bad[2] == 0.0         # ast roto


def test_train_logreg_learns_separable():
    X = [[0, 0], [0, 1], [1, 0], [1, 1], [5, 5], [6, 5], [5, 6], [6, 6]]
    y = [0, 0, 0, 0, 1, 1, 1, 1]
    m = train_logreg(X, y, epochs=500)
    assert accuracy(m, X, y) == 1.0


def test_train_code_quality_separates():
    good = ["def add(a, b):\n    \"\"\"suma\"\"\"\n    return a + b\n\ndef test_add():\n    assert add(1, 2) == 3\n"] * 6
    bad = ["def f(:\n", "x=1;y;z"] * 6
    samples = [(c, 1) for c in good] + [(c, 0) for c in bad]
    r = train_code_quality(samples)
    assert r["n"] == 18 and r["train_acc"] >= 0.9
    assert r["predict"](good[0]) > r["predict"](bad[0])   # buen > mal


def test_emit_training_job_does_not_run():
    job = emit_training_job(model_kind="jepa-tiny", dataset_ref="github://x", hardware="gpu")
    assert job["status"] == "emitted" and job["hardware"] == "gpu"
    assert "no lo entrena local" in job["note"]
