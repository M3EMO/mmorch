"""Fase 5: shadow prior contextual sobre el bandit. Checks de la spec Q5.
Embeddings mockeados (deterministas, cero API): el contexto codifica su propio vector."""
import sys, pathlib, random
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.shadow_prior as SP
from mmorch.feedback import ThompsonBandit


def _fake_embed(text):
    """Embedding determinista: 'a:0.9' -> vector que depende de la familia del contexto.
    Contextos que empiezan igual quedan cerca en coseno."""
    h = text.split(":")[0]
    base = {"img": [1.0, 0.0, 0.0], "sql": [0.0, 1.0, 0.0], "math": [0.0, 0.0, 1.0]}
    return base.get(h, [0.3, 0.3, 0.3])


def test_scale_zero_identical_to_pure_bandit(monkeypatch, tmp_path):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    b = ThompsonBandit(path=tmp_path / "b.json")
    b.update("img:gen", 1.0); b.update("sql:gen", 0.0); b.update("math:gen", 1.0)
    arms = ["img:gen", "sql:gen", "math:gen"]
    prior = SP.ShadowPrior.from_outcomes(scale=0.0, outcomes=[
        {"arm": "img:gen", "reward": 1.0, "context": "img:1"},
        {"arm": "sql:gen", "reward": 0.0, "context": "sql:1"},
        {"arm": "math:gen", "reward": 1.0, "context": "math:1"},
    ])
    # mismo seed -> misma secuencia de betavariate -> misma eleccion que bandit puro
    for seed in range(20):
        a = prior.select(b, arms, context="img:9", rng=random.Random(seed))
        c = b.select(arms, rng=random.Random(seed))
        assert a == c, f"scale=0 debe ser identico al bandit puro (seed {seed})"


def test_prior_abstains_without_neighbors(monkeypatch):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    prior = SP.ShadowPrior(scale=0.5)
    prior.index = {"img:gen": [([1.0, 0, 0], 1.0)]}   # 1 punto < _MIN_NEIGHBORS
    ap, bp = prior.prior_for("img:gen", [1.0, 0, 0])
    assert (ap, bp) == (0.0, 0.0)


def test_prior_pseudocounts_follow_context(monkeypatch):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    # brazo con 4 contextos 'img' que SIEMPRE dieron reward 1 -> prior alpha>>beta pa ctx img
    prior = SP.ShadowPrior(scale=0.5)
    prior.index = {"g": [([1.0, 0, 0], 1.0)] * 4}
    ap, bp = prior.prior_for("g", [1.0, 0, 0])
    assert ap > bp and ap > 0


def test_offline_improvement_positive_when_context_predicts(monkeypatch):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    # contexto predice reward: img->1, sql->0 (mismo brazo). El prior debe batir la media global.
    outs = []
    for _ in range(6):
        outs.append({"arm": "g", "reward": 1.0, "context": "img:x"})
        outs.append({"arm": "g", "reward": 0.0, "context": "sql:x"})
    imp = SP.offline_improvement(outs)
    assert imp > 0.02, f"prior deberia mejorar Brier (got {imp})"


def test_auto_scale_respects_bounds(monkeypatch):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    outs = []
    for _ in range(6):
        outs.append({"arm": "g", "reward": 1.0, "context": "img:x"})
        outs.append({"arm": "g", "reward": 0.0, "context": "sql:x"})
    # sube desde 0 (mejora) pero nunca por debajo de SCALE_MIN cuando sube
    new, gate = SP.auto_scale(0.0, outs)
    assert SP.SCALE_MIN <= new <= SP.SCALE_MAX and not gate
    # en el tope, querer subir mas pide gate humano
    new2, gate2 = SP.auto_scale(SP.SCALE_MAX, outs)
    assert new2 == SP.SCALE_MAX and gate2 is True


def test_embed_fn_pluggable_overrides_module_embed(monkeypatch):
    # el module-level embed devuelve algo INUTIL; embed_fn custom es el que debe usarse
    monkeypatch.setattr(SP, "embed", lambda t: [0.0, 0.0, 0.0])
    custom = _fake_embed
    sp = SP.ShadowPrior.from_outcomes(scale=0.5, embed_fn=custom, outcomes=[
        {"arm": "g", "reward": 1.0, "context": "img:1"},
        {"arm": "g", "reward": 1.0, "context": "img:2"},
        {"arm": "g", "reward": 1.0, "context": "img:3"},
    ])
    # si uso custom, los 3 puntos 'img' estan juntos -> prior fuerte alpha>beta
    ap, bp = sp.prior_for("g", custom("img:9"))
    assert ap > bp and ap > 0


def test_auto_scale_drops_when_no_signal(monkeypatch):
    monkeypatch.setattr(SP, "embed", _fake_embed)
    # contexto NO predice (reward aleatorio respecto a ctx) -> no sube
    outs = [{"arm": "g", "reward": (i % 2), "context": "img:x"} for i in range(12)]
    new, gate = SP.auto_scale(0.3, outs)
    assert new <= 0.3 and not gate
