"""Deterministic post-merge observation policy for autonomous promotions."""

from __future__ import annotations

from dataclasses import dataclass


REQUIRED_BOOLEAN = ("tests", "gates", "smoke", "health")


class ObservationError(RuntimeError):
    """An observation is malformed; callers must fail closed."""


@dataclass(frozen=True)
class ObservationVerdict:
    action: str  # wait | accept | revert
    reasons: list[str]
    noisy_failures: list[str]


def validate_sample(sample: dict) -> dict:
    if not isinstance(sample, dict):
        raise ObservationError("observation sample must be an object")
    missing = [k for k in (*REQUIRED_BOOLEAN, "canary", "error_rate", "cost_per_call") if k not in sample]
    if missing:
        raise ObservationError(f"observation missing signals: {missing}")
    if any(sample[k] is not True and sample[k] is not False for k in REQUIRED_BOOLEAN):
        raise ObservationError("tests/gates/smoke/health must be explicit booleans")
    if not isinstance(sample["canary"], dict):
        raise ObservationError("canary must map model to pass rate")
    try:
        canary = {str(k): float(v) for k, v in sample["canary"].items()}
        error_rate = float(sample["error_rate"])
        cost_per_call = float(sample["cost_per_call"])
    except (TypeError, ValueError) as exc:
        raise ObservationError(f"non-numeric observation signal: {exc}") from exc
    if any(not 0.0 <= v <= 1.0 for v in canary.values()) or not 0.0 <= error_rate <= 1.0:
        raise ObservationError("canary and error_rate must be in [0,1]")
    if cost_per_call < 0:
        raise ObservationError("cost_per_call must be non-negative")
    return {
        **sample,
        "canary": canary,
        "error_rate": error_rate,
        "cost_per_call": cost_per_call,
    }


def _noisy_failures(
    baseline: dict,
    current: dict,
    *,
    canary_drop: float,
    error_rate_increase: float,
    cost_ratio: float,
) -> list[str]:
    failures: list[str] = []
    for model, prior in baseline["canary"].items():
        now = current["canary"].get(model)
        if now is None:
            failures.append(f"canary_missing:{model}")
        elif now < prior - canary_drop:
            failures.append(f"canary_drop:{model}")
    if current["error_rate"] > baseline["error_rate"] + error_rate_increase:
        failures.append("error_rate")
    base_cost = baseline["cost_per_call"]
    if base_cost == 0:
        if current["cost_per_call"] > 0:
            failures.append("cost_per_call")
    elif current["cost_per_call"] > base_cost * cost_ratio:
        failures.append("cost_per_call")
    return failures


def evaluate(
    baseline: dict,
    samples: list[dict],
    *,
    started_at: float,
    now: float,
    horizon_s: float = 26 * 3600,
    min_samples: int = 2,
    canary_drop: float = 0.1,
    error_rate_increase: float = 0.05,
    cost_ratio: float = 1.25,
) -> ObservationVerdict:
    """Accept only after a healthy horizon; deterministic failure reverts immediately."""
    base = validate_sample(baseline)
    if min_samples < 1 or horizon_s <= 0:
        raise ObservationError("min_samples and horizon_s must be positive")
    if not samples:
        if now - started_at >= horizon_s:
            return ObservationVerdict("revert", ["observation_deadline_without_samples"], [])
        return ObservationVerdict("wait", ["awaiting_samples"], [])

    clean = [validate_sample(s) for s in samples]
    latest = clean[-1]
    deterministic = [k for k in REQUIRED_BOOLEAN if latest[k] is False]
    if deterministic:
        return ObservationVerdict(
            "revert",
            [f"deterministic:{name}" for name in deterministic],
            [],
        )

    latest_noisy = _noisy_failures(
        base,
        latest,
        canary_drop=canary_drop,
        error_rate_increase=error_rate_increase,
        cost_ratio=cost_ratio,
    )
    previous_noisy = (
        _noisy_failures(
            base,
            clean[-2],
            canary_drop=canary_drop,
            error_rate_increase=error_rate_increase,
            cost_ratio=cost_ratio,
        )
        if len(clean) >= 2
        else []
    )
    repeated = sorted(set(latest_noisy) & set(previous_noisy))
    if repeated:
        return ObservationVerdict(
            "revert",
            [f"repeated_noisy:{name}" for name in repeated],
            latest_noisy,
        )

    elapsed = now - started_at
    if elapsed >= horizon_s and len(clean) >= min_samples and not latest_noisy:
        return ObservationVerdict("accept", [], [])
    if elapsed >= horizon_s:
        reasons = ["observation_deadline_unhealthy"] if latest_noisy else ["insufficient_samples"]
        return ObservationVerdict("revert", reasons, latest_noisy)
    return ObservationVerdict("wait", ["horizon_not_complete"], latest_noisy)
