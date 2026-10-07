"""PROTOTYPE: typed local update-rule search with learning rate in the genome."""

from __future__ import annotations

import argparse
import json
from typing import Any

import numpy as np

from benchmark import FAMILIES, datasets
from prototype import Network, calibrate, mse, random_network, relax_batch


Expr = str | float | tuple
BETA = 0.25
TIE_ABS = 1e-12
TIE_REL = 1e-8
CANONICAL_EQPROP: Expr = ("neg", "phase")
SIGN_EQPROP: Expr = ("neg", ("sign", "phase"))
EPOCHS = 8


def tied(a: float, b: float) -> bool:
    scale = max(abs(a), abs(b), 1e-12)
    return abs(a - b) <= max(TIE_ABS, TIE_REL * scale)


def canonicalize(expr: Expr) -> Expr:
    if isinstance(expr, (float, int)):
        return float(expr)
    if isinstance(expr, str):
        return expr
    op, *raw = expr
    args = [canonicalize(item) for item in raw]
    if op == "neg":
        inner = args[0]
        if isinstance(inner, tuple) and inner[0] == "neg":
            return inner[1]
        if isinstance(inner, float):
            return -inner
        return ("neg", inner)
    if op == "sign":
        inner = args[0]
        if isinstance(inner, tuple) and inner[0] == "sign":
            return inner
        if isinstance(inner, tuple) and inner[0] == "neg":
            return canonicalize(("neg", ("sign", inner[1])))
        return ("sign", inner)
    if op == "clip":
        inner = args[0]
        if isinstance(inner, tuple) and inner[0] == "clip":
            return inner
        return ("clip", inner)
    if op in ("add", "mul"):
        left, right = args
        if repr(left) > repr(right):
            left, right = right, left
        return (op, left, right)
    if op == "sub":
        return ("sub", args[0], args[1])
    raise ValueError(f"unknown typed primitive: {op}")


def expr_key(expr: Expr) -> str:
    return repr(canonicalize(expr))


def evaluate_expr(expr: Expr, features: dict[str, np.ndarray]) -> np.ndarray:
    if isinstance(expr, str):
        return features[expr]
    if isinstance(expr, (float, int)):
        return np.full_like(features["phase"], float(expr))
    op, *args = expr
    values = [evaluate_expr(arg, features) for arg in args]
    if op == "neg":
        return -values[0]
    if op == "sign":
        return np.sign(values[0])
    if op == "clip":
        return np.clip(values[0], -1.0, 1.0)
    if op == "add":
        return values[0] + values[1]
    if op == "sub":
        return values[0] - values[1]
    if op == "mul":
        return values[0] * values[1]
    raise ValueError(f"unknown typed primitive: {op}")


def random_expr(rng: np.random.Generator, depth: int = 2) -> Expr:
    terminals: tuple[Expr, ...] = ("phase", "shift", "free_energy", "g", 0.1, -0.1, 0.5)
    if depth <= 0 or rng.random() < 0.35:
        return terminals[int(rng.integers(len(terminals)))]
    if rng.random() < 0.45:
        op = ("neg", "sign", "clip")[int(rng.integers(3))]
        return (op, random_expr(rng, depth - 1))
    op = ("add", "sub", "mul")[int(rng.integers(3))]
    return (op, random_expr(rng, depth - 1), random_expr(rng, depth - 1))


def random_lr(rng: np.random.Generator) -> float:
    return float(10 ** rng.uniform(-3.0, -0.3))


def mutate_expr(expr: Expr, rng: np.random.Generator) -> Expr:
    if rng.random() < 0.5:
        return random_expr(rng)
    return canonicalize(("neg", expr) if rng.random() < 0.5 else ("sign", expr))


def mutate_lr(learning_rate: float, rng: np.random.Generator) -> float:
    return float(np.clip(learning_rate * np.exp(rng.normal(0.0, 0.4)), 1e-4, 1.0))


def adapt(
    initial: Network,
    rule: Expr | None,
    inputs: np.ndarray,
    expected: np.ndarray,
    *,
    epochs: int = EPOCHS,
    learning_rate: float = 0.06,
) -> tuple[Network, list[float]]:
    net, _, _ = calibrate(initial, inputs, expected)
    mask = net.conductance > 0
    curve = [mse(net, inputs, expected)]
    if rule is None:
        return net, curve
    denominator = net.scale if abs(net.scale) >= 1e-6 else 1e-6
    raw_target = np.clip((expected - net.bias) / denominator, -2.0, 2.0)
    for _ in range(epochs):
        free, _ = relax_batch(net, inputs)
        nudged, _ = relax_batch(net, inputs, nudge_targets=raw_target, beta=BETA)
        free_delta = free[:, :, None] - free[:, None, :]
        nudged_delta = nudged[:, :, None] - nudged[:, None, :]
        features = {
            "phase": np.mean(
                (nudged_delta**2 - free_delta**2) / (2.0 * BETA),
                axis=0,
            ),
            "shift": np.mean(nudged_delta - free_delta, axis=0),
            "free_energy": np.mean(free_delta**2, axis=0),
            "g": net.conductance / max(float(net.conductance.max()), 1e-9),
        }
        update = np.asarray(evaluate_expr(rule, features), dtype=float)
        update = np.nan_to_num(update, nan=0.0, posinf=1.0, neginf=-1.0)
        update = np.clip((update + update.T) / 2.0, -1.0, 1.0)
        net.conductance[mask] = np.clip(
            net.conductance[mask] + learning_rate * update[mask],
            0.02,
            3.0,
        )
        np.fill_diagonal(net.conductance, 0.0)
        curve.append(mse(net, inputs, expected))
    return net, curve


def task_score(
    rule: Expr | None,
    *,
    family_index: int,
    seed: int,
    nodes: int = 6,
    learning_rate: float = 0.06,
) -> float:
    family = FAMILIES[family_index]
    train_x, _, _ = datasets(family, seed)
    expected = family.fn(train_x)
    initial = random_network(np.random.default_rng(seed + family_index * 100), nodes=nodes)
    _, baseline, _ = calibrate(initial, train_x, expected)
    learned, _ = adapt(initial, rule, train_x, expected, learning_rate=learning_rate)
    return mse(learned, train_x, expected) / max(baseline, 1e-12)


def meta_score(rule: Expr | None, learning_rate: float = 0.06) -> float:
    try:
        ratios = [
            task_score(
                rule,
                family_index=family_index,
                seed=seed,
                learning_rate=learning_rate,
            )
            for family_index in (0, 1)
            for seed in (7, 19)
        ]
        return float(np.mean(np.log(np.clip(ratios, 1e-6, 1e6))))
    except (FloatingPointError, RuntimeError, ValueError, np.linalg.LinAlgError):
        return float("inf")


def tune_lr(
    expr: Expr,
    rng: np.random.Generator,
    budget: int,
) -> tuple[float, float, int]:
    best_lr = 0.06
    best_score = float("inf")
    used = 0
    for _ in range(budget):
        candidate = random_lr(rng)
        score = meta_score(expr, candidate)
        used += 1
        if score < best_score:
            best_score = score
            best_lr = candidate
    return best_lr, best_score, used


def discover(seed: int, candidates: int) -> tuple[Expr, float, dict]:
    rng = np.random.default_rng(seed)
    scored: list[tuple[float, Expr, float]] = []
    seen: set[str] = set()
    population: list[tuple[Expr, float]] = []
    while len(population) < min(8, candidates):
        expr = canonicalize(random_expr(rng))
        lr = random_lr(rng)
        key = f"{expr_key(expr)}|{lr:.4g}"
        if key in seen:
            continue
        seen.add(key)
        population.append((expr, lr))
    for expr, lr in population:
        scored.append((meta_score(expr, lr), expr, lr))
    used = len(scored)
    attempts = 0
    while used < candidates and attempts < candidates * 20:
        attempts += 1
        _parent_score, parent_expr, parent_lr = scored[int(rng.integers(len(scored)))]
        if rng.random() < 0.5:
            child_expr = canonicalize(mutate_expr(parent_expr, rng))
            child_lr = parent_lr
        else:
            child_expr = parent_expr
            child_lr = mutate_lr(parent_lr, rng)
        key = f"{expr_key(child_expr)}|{child_lr:.4g}"
        if key in seen:
            continue
        seen.add(key)
        scored.append((meta_score(child_expr, child_lr), child_expr, child_lr))
        used += 1
    score, winner, lr = min(scored, key=lambda item: item[0])
    return winner, lr, {
        "candidate_programs": used,
        "meta_train_log_ratio": score,
        "unique_canonical_exprs": len({expr_key(item[1]) for item in scored}),
    }


def heldout_case(
    rule: Expr | None,
    family_index: int,
    seed: int,
    nodes: int,
    learning_rate: float,
) -> dict:
    family = FAMILIES[family_index]
    train_x, id_x, ood_x = datasets(family, seed + nodes * 100)
    train_y, id_y, ood_y = family.fn(train_x), family.fn(id_x), family.fn(ood_x)
    initial = random_network(np.random.default_rng(seed), nodes=nodes)
    baseline_net, _, _ = calibrate(initial, train_x, train_y)
    learned, curve = adapt(
        initial,
        rule,
        train_x,
        train_y,
        learning_rate=learning_rate,
    )
    result = {
        "train_before": curve[0],
        "train_after": curve[-1],
        "id_mse": mse(learned, id_x, id_y),
        "ood_mse": mse(learned, ood_x, ood_y),
        "baseline_ood_mse": mse(baseline_net, ood_x, ood_y),
        "learning_rate": learning_rate,
    }
    perturb = learned.copy()
    rng = np.random.default_rng(seed + 99_000)
    noise = rng.normal(0, 0.05, size=perturb.conductance.shape)
    noise = (noise + noise.T) / 2
    perturb.conductance *= np.clip(1.0 + noise, 0.0, None)
    result["perturbed_ood_mse"] = mse(perturb, ood_x, ood_y)
    return result


def mean_ood_ties(mean_ood: dict[str, float]) -> list[str]:
    best = min(mean_ood.values())
    return sorted(name for name, value in mean_ood.items() if tied(value, best))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=5)
    parser.add_argument("--candidates", type=int, default=16)
    args = parser.parse_args()
    discovered, discovered_lr, search = discover(args.seed, args.candidates)
    tune_rng = np.random.default_rng(args.seed + 17)
    canonical_lr, canonical_meta, canonical_used = tune_lr(
        CANONICAL_EQPROP,
        tune_rng,
        args.candidates,
    )
    sign_lr, sign_meta, sign_used = tune_lr(SIGN_EQPROP, tune_rng, args.candidates)
    no_learning_meta = meta_score(None)
    search["control_meta_evals"] = {
        "canonical_eqprop": canonical_used,
        "sign_eqprop": sign_used,
        "discovered": search["candidate_programs"],
        "no_learning": 1,
    }
    search["canonical_log_ratio"] = canonical_meta
    search["sign_eqprop_log_ratio"] = sign_meta
    search["no_learning_log_ratio"] = no_learning_meta
    rules: dict[str, tuple[Expr | None, float]] = {
        "discovered": (discovered, discovered_lr),
        "canonical_eqprop": (CANONICAL_EQPROP, canonical_lr),
        "sign_eqprop": (SIGN_EQPROP, sign_lr),
        "no_learning": (None, 0.0),
    }
    cases: list[dict[str, Any]] = []
    for family_index in range(len(FAMILIES)):
        for nodes in (6, 10):
            for seed in (31, 43):
                results = {
                    name: heldout_case(rule, family_index, seed, nodes, lr)
                    for name, (rule, lr) in rules.items()
                }
                oods = {name: item["ood_mse"] for name, item in results.items()}
                best = min(oods.values())
                winners = sorted(name for name, value in oods.items() if tied(value, best))
                cases.append({
                    "family": FAMILIES[family_index].name,
                    "nodes": nodes,
                    "seed": seed,
                    "winners": winners,
                    "tie": len(winners) > 1,
                    "results": results,
                })
    mean_ood = {
        name: float(np.mean([case["results"][name]["ood_mse"] for case in cases]))
        for name in rules
    }
    tied_names = mean_ood_ties(mean_ood)
    discovered_canon = expr_key(discovered)
    algebraic_control = discovered_canon in {
        expr_key(CANONICAL_EQPROP),
        expr_key(SIGN_EQPROP),
    }
    beats_tuned_sign = mean_ood["discovered"] < mean_ood["sign_eqprop"] and not tied(
        mean_ood["discovered"],
        mean_ood["sign_eqprop"],
    )
    beats_tuned_canonical = mean_ood["discovered"] < mean_ood["canonical_eqprop"] and not tied(
        mean_ood["discovered"],
        mean_ood["canonical_eqprop"],
    )
    novelty = bool(
        (not algebraic_control)
        and beats_tuned_sign
        and beats_tuned_canonical
        and tied_names == ["discovered"]
    )
    output = {
        "grammar": {
            "features": ["phase", "shift", "free_energy", "g"],
            "unary": ["neg", "sign", "clip"],
            "binary": ["add", "sub", "mul"],
            "learning_rate_in_genome": True,
        },
        "discovered_rule": discovered,
        "discovered_canonical": canonicalize(discovered),
        "discovered_lr": discovered_lr,
        "control_lrs": {
            "canonical_eqprop": canonical_lr,
            "sign_eqprop": sign_lr,
        },
        "algebraic_match_to_control": algebraic_control,
        "search": search,
        "heldout": {
            "cases": len(cases),
            "mean_ood_mse": mean_ood,
            "tied_best_by_mean_ood": tied_names,
            "novelty": novelty,
            "novelty_checks": {
                "not_algebraic_control": not algebraic_control,
                "beats_tuned_sign_eqprop": beats_tuned_sign,
                "beats_tuned_canonical_eqprop": beats_tuned_canonical,
                "unique_mean_winner": tied_names == ["discovered"],
            },
        },
        "case_details": cases,
    }
    print(json.dumps(output, indent=2))
    pipeline_ok = (
        search["control_meta_evals"]["canonical_eqprop"] == args.candidates
        and search["control_meta_evals"]["sign_eqprop"] == args.candidates
        and search["control_meta_evals"]["discovered"] == search["candidate_programs"]
    )
    if not pipeline_ok:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
