"""weights: model-cards + verificacion sha256 de los pesos de nodos neuronales.
Happy path usa el manifest real (sha matchea el .npz committeado); tamper usa tmp."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.weights as W


def test_card_has_arch_and_sha():
    c = W.card("code_embedder")
    assert c and c["out_dim"] in (256, 384) and len(c["sha256"]) == 64
    assert "regen_cmd" in c and "metric" in c   # model-card completo


def test_list_weights():
    assert "code_embedder" in W.list_weights()


def test_verify_real_weight_matches():
    ok, detail = W.verify("code_embedder")
    assert ok, f"el .npz committeado deberia matchear el manifest: {detail}"


def test_resolve_returns_path_when_valid():
    p = W.resolve("code_embedder")
    assert p.endswith("code_embedder.npz")


def test_verify_detects_tamper(tmp_path):
    # manifest con sha WRONG apuntando al npz real -> verify falla (detecta tamper/corrupcion)
    W.card("code_embedder")["path"]
    bad = tmp_path / "manifest.json"
    bad.write_text(json.dumps({"code_embedder": {**W.card("code_embedder"),
                   "sha256": "0" * 64}}), encoding="utf-8")
    ok, detail = W.verify("code_embedder", path=bad)
    assert not ok and "NO matchea" in detail


def test_resolve_raises_on_bad_sha(tmp_path):
    bad = tmp_path / "m.json"
    bad.write_text(json.dumps({"code_embedder": {**W.card("code_embedder"),
                   "sha256": "0" * 64}}), encoding="utf-8")
    try:
        W.resolve("code_embedder", path=bad)
        assert False, "deberia lanzar con sha invalido"
    except ValueError as e:
        assert "fallo" in str(e)


def test_unknown_weight():
    assert W.card("ghost") is None
    ok, _ = W.verify("ghost")
    assert ok is False
