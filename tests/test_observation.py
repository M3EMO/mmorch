import pytest

from mmorch.observation import ObservationError, evaluate


def sample(**overrides):
    value = {
        "tests": True,
        "gates": True,
        "smoke": True,
        "health": True,
        "canary": {"deepseek": 0.9, "gemini": 0.9},
        "error_rate": 0.01,
        "cost_per_call": 0.001,
    }
    value.update(overrides)
    return value


def test_healthy_samples_accept_only_after_horizon():
    base = sample()
    samples = [sample(), sample()]
    assert evaluate(base, samples, started_at=0, now=100).action == "wait"
    assert evaluate(base, samples, started_at=0, now=26 * 3600).action == "accept"


@pytest.mark.parametrize("signal", ["tests", "gates", "smoke", "health"])
def test_deterministic_regression_reverts_immediately(signal):
    verdict = evaluate(sample(), [sample(**{signal: False})], started_at=0, now=1)
    assert verdict.action == "revert"
    assert verdict.reasons == [f"deterministic:{signal}"]


def test_single_noisy_canary_drop_waits_but_repeated_drop_reverts():
    degraded = sample(canary={"deepseek": 0.7, "gemini": 0.9})
    one = evaluate(sample(), [degraded], started_at=0, now=10)
    assert one.action == "wait"
    assert one.noisy_failures == ["canary_drop:deepseek"]

    two = evaluate(sample(), [degraded, degraded], started_at=0, now=20)
    assert two.action == "revert"
    assert two.reasons == ["repeated_noisy:canary_drop:deepseek"]


@pytest.mark.parametrize(
    "degraded",
    [
        sample(error_rate=0.2),
        sample(cost_per_call=0.002),
        sample(canary={"deepseek": 0.9}),
    ],
)
def test_repeated_operational_regression_reverts(degraded):
    verdict = evaluate(sample(), [degraded, degraded], started_at=0, now=20)
    assert verdict.action == "revert"
    assert verdict.noisy_failures


def test_no_samples_at_deadline_reverts():
    verdict = evaluate(sample(), [], started_at=0, now=26 * 3600)
    assert verdict.action == "revert"
    assert verdict.reasons == ["observation_deadline_without_samples"]


def test_too_few_samples_at_deadline_reverts():
    verdict = evaluate(sample(), [sample()], started_at=0, now=26 * 3600)
    assert verdict.action == "revert"
    assert verdict.reasons == ["insufficient_samples"]


def test_noisy_latest_sample_at_deadline_reverts_without_waiting_forever():
    degraded = sample(error_rate=0.2)
    verdict = evaluate(sample(), [sample(), degraded], started_at=0, now=26 * 3600)
    assert verdict.action == "revert"
    assert verdict.reasons == ["observation_deadline_unhealthy"]


@pytest.mark.parametrize(
    "bad",
    [
        {},
        sample(tests=None),
        sample(canary=[]),
        sample(error_rate=2),
        sample(cost_per_call=-1),
    ],
)
def test_malformed_or_ambiguous_sample_fails_closed(bad):
    with pytest.raises(ObservationError):
        evaluate(sample(), [bad], started_at=0, now=1)
