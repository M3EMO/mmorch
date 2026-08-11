"""ticket 03: ensemble_verify paraleliza los K verificadores (ThreadPoolExecutor) — el
orden de verdicts debe seguir siendo por indice de verifier_models, sin importar el
orden real de finalizacion de los threads."""
import sys, pathlib, time
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.ensemble as EN
from mmorch.patterns import Verdict


def test_ensemble_verify_preserves_order_under_uneven_latency(monkeypatch):
    # el primer verificador tarda mas que el segundo -> si el orden dependiera del
    # orden de finalizacion (no del indice), verdicts saldria invertido.
    delays = {"gemini-2.5-flash": 0.08, "kimi-k2.5": 0.0}

    def _fake_av(art, *, gen_model, verifier_model, rubric=None, phase=""):
        time.sleep(delays.get(verifier_model, 0.0))
        return Verdict(passed=True, confidence=0.9, refutations=[], raw="",
                       verifier_model=verifier_model, cost_usd=0.0)

    monkeypatch.setattr(EN, "adversarial_verify", _fake_av)
    ev = EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat",
                            verifier_models=["gemini-2.5-flash", "kimi-k2.5"])
    assert [v.verifier_model for v in ev.verdicts] == ["gemini-2.5-flash", "kimi-k2.5"]


def test_ensemble_verify_parallel_matches_serial_result(monkeypatch):
    """Equivalencia: mismo input -> mismo EnsembleVerdict que la version serial
    (list comprehension), solo mas rapido en wall-clock."""
    verifier_models = ["gemini-2.5-flash", "gemini-2.5-flash-lite", "kimi-k2.5"]
    outcomes = {"gemini-2.5-flash": (True, 0.9), "gemini-2.5-flash-lite": (False, 0.3),
                "kimi-k2.5": (True, 0.7)}

    def _fake_av(art, *, gen_model, verifier_model, rubric=None, phase=""):
        passed, conf = outcomes[verifier_model]
        refs = [] if passed else ["nope"]
        return Verdict(passed=passed, confidence=conf, refutations=refs, raw="",
                       verifier_model=verifier_model, cost_usd=0.01)

    monkeypatch.setattr(EN, "adversarial_verify", _fake_av)
    parallel = EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat",
                                  verifier_models=verifier_models)

    # version serial de referencia (pre-paralelizacion), misma logica de agregacion.
    serial_verdicts = [_fake_av("x", gen_model="deepseek-chat", verifier_model=vm, rubric="r")
                       for vm in verifier_models]
    n_pass = sum(1 for v in serial_verdicts if v.passed)
    serial_passed = n_pass > len(serial_verdicts) / 2

    assert parallel.passed == serial_passed
    assert parallel.n_passed == n_pass
    assert [v.verifier_model for v in parallel.verdicts] == verifier_models
    assert [v.passed for v in parallel.verdicts] == [v.passed for v in serial_verdicts]
    assert parallel.refutations == [r for v in serial_verdicts if not v.passed for r in v.refutations]


def test_ensemble_verify_faster_than_serial_would_be(monkeypatch):
    """Sanity de que efectivamente corre en paralelo (no solo preserva orden): 3
    verificadores de ~0.1s c/u deben completar en bastante menos que 0.3s seriales."""
    def _fake_av(art, *, gen_model, verifier_model, rubric=None, phase=""):
        time.sleep(0.1)
        return Verdict(passed=True, confidence=0.9, refutations=[], raw="",
                       verifier_model=verifier_model, cost_usd=0.0)

    monkeypatch.setattr(EN, "adversarial_verify", _fake_av)
    t0 = time.monotonic()
    EN.ensemble_verify("x", rubric="r", gen_model="deepseek-chat",
                       verifier_models=["gemini-2.5-flash", "gemini-2.5-flash-lite", "kimi-k2.5"])
    elapsed = time.monotonic() - t0
    assert elapsed < 0.25, f"esperado paralelo <0.25s, tardo {elapsed:.3f}s"
