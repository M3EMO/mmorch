"""ticket 21: _cosine_batch (1 matmul N x dim) debe dar el mismo resultado que llamar
_cosine fila a fila, incluyendo el caso vector-cero."""
import sys, pathlib, random
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.memory as M


def test_cosine_batch_matches_per_row_cosine():
    random.seed(0)
    qvec = [random.uniform(-1, 1) for _ in range(384)]
    embs = [[random.uniform(-1, 1) for _ in range(384)] for _ in range(37)]

    batch = M._cosine_batch(qvec, embs)
    serial = [M._cosine(qvec, e) for e in embs]

    assert len(batch) == len(serial) == 37
    for b, s in zip(batch, serial, strict=True):
        assert abs(b - s) < 1e-5, (b, s)


def test_cosine_batch_zero_vector_matches_serial():
    qvec = [1.0, 0.0, 0.0]
    embs = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    batch = M._cosine_batch(qvec, embs)
    serial = [M._cosine(qvec, e) for e in embs]
    assert batch == [0.0, 1.0, 0.0]
    assert [round(x, 6) for x in batch] == [round(x, 6) for x in serial]


def test_cosine_batch_zero_query_returns_all_zero():
    embs = [[1.0, 2.0, 3.0], [0.0, 0.0, 0.0]]
    assert M._cosine_batch([0.0, 0.0, 0.0], embs) == [0.0, 0.0]


def test_cosine_batch_empty():
    assert M._cosine_batch([1.0, 0.0], []) == []
