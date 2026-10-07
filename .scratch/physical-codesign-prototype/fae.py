"""PROTOTYPE: FAE pair — one fitness over software genome and hardware genome."""

from __future__ import annotations

import argparse
import json

import numpy as np

from benchmark import FAMILIES, datasets
from learning_rules import (
    SIGN_EQPROP,
    adapt,
    canonicalize,
    expr_key,
    mutate_expr,
    mutate_lr,
    random_expr,
    random_lr,
)
from movement import MSE_EDGE_PENALTY, accounting
from prototype import calibrate, mse, mutate, random_network, raw_predictions


ARMS = ("hw_only", "sw_only", "fae", "random")


def objective(ood_mse: float, edges: int, steps: int) -> float:
    return ood_mse + MSE_EDGE_PENALTY * edges * steps


def evaluate(
    net,
    rule,
    learning_rate: float,
    train_x: np.ndarray,
    train_y: np.ndarray,
    ood_x: np.ndarray,
    ood_y: np.ndarray,
) -> tuple[object, dict]:
    learned, _ = adapt(net, rule, train_x, train_y, learning_rate=learning_rate)
    _, physics = raw_predictions(learned, ood_x)
    costs = accounting(learned, ood_x, int(physics["steps"]))
    ood_mse = mse(learned, ood_x, ood_y)
    return learned, {
        "ood_mse": ood_mse,
        "train_mse": mse(learned, train_x, train_y),
        "objective": objective(ood_mse, costs["edges"], costs["steps"]),
        "hops": costs["hops"],
        "edges": costs["edges"],
        "steps": costs["steps"],
        "rule": canonicalize(rule) if rule is not None else None,
        "learning_rate": learning_rate,
    }


def propose(mode: str, net, rule, learning_rate: float, rng: np.random.Generator):
    if mode == "random":
        return random_network(rng, nodes=len(net.leak)), canonicalize(random_expr(rng)), random_lr(rng)
    if mode == "hw_only":
        return mutate(net, rng, mode="co_design"), SIGN_EQPROP, mutate_lr(learning_rate, rng)
    if mode == "sw_only":
        return net.copy(), canonicalize(mutate_expr(rule, rng)), mutate_lr(learning_rate, rng)
    which = int(rng.integers(3))
    if which == 0:
        return mutate(net, rng, mode="co_design"), rule, learning_rate
    if which == 1:
        return net.copy(), canonicalize(mutate_expr(rule, rng)), learning_rate
    return net.copy(), rule, mutate_lr(learning_rate, rng)


def optimize_arm(
    mode: str,
    initial,
    *,
    rng: np.random.Generator,
    iterations: int,
    train_x: np.ndarray,
    train_y: np.ndarray,
    ood_x: np.ndarray,
    ood_y: np.ndarray,
) -> dict:
    net = initial.copy()
    rule = SIGN_EQPROP
    lr = 0.06
    if mode == "sw_only":
        rule = canonicalize(random_expr(rng))
        lr = random_lr(rng)
    elif mode == "fae":
        rule = canonicalize(random_expr(rng))
        lr = random_lr(rng)
    elif mode == "random":
        net = random_network(rng, nodes=len(initial.leak))
        rule = canonicalize(random_expr(rng))
        lr = random_lr(rng)
    _, best = evaluate(net, rule, lr, train_x, train_y, ood_x, ood_y)
    accepted = 0
    for _ in range(iterations):
        proposal_net, proposal_rule, proposal_lr = propose(mode, net, rule, lr, rng)
        _, metrics = evaluate(
            proposal_net,
            proposal_rule,
            proposal_lr,
            train_x,
            train_y,
            ood_x,
            ood_y,
        )
        if metrics["objective"] < best["objective"]:
            net, rule, lr, best = proposal_net, proposal_rule, proposal_lr, metrics
            accepted += 1
    return {
        "evaluations": iterations,
        "accepted_mutations": accepted,
        "rule_key": None if best["rule"] is None else expr_key(best["rule"]),
        **{k: v for k, v in best.items() if k != "rule"},
        "rule": best["rule"],
    }


def run_case(family_index: int, seed: int, nodes: int, iterations: int) -> dict:
    family = FAMILIES[family_index]
    train_x, _, ood_x = datasets(family, seed + nodes * 100)
    train_y, ood_y = family.fn(train_x), family.fn(ood_x)
    initial, _, _ = calibrate(
        random_network(np.random.default_rng(seed), nodes=nodes),
        train_x,
        train_y,
    )
    variants = {}
    for offset, mode in enumerate(ARMS, start=1):
        variants[mode] = optimize_arm(
            mode,
            initial,
            rng=np.random.default_rng(seed + nodes * 10_000 + offset * 1_000),
            iterations=iterations,
            train_x=train_x,
            train_y=train_y,
            ood_x=ood_x,
            ood_y=ood_y,
        )
    winner = min(ARMS, key=lambda mode: variants[mode]["objective"])
    best_control = min(variants[mode]["objective"] for mode in ARMS if mode != "fae")
    return {
        "family": family.name,
        "nodes": nodes,
        "seed": seed,
        "evaluations_per_arm": iterations,
        "winner_objective": winner,
        "fae_vs_best_control": variants["fae"]["objective"] / best_control,
        "variants": variants,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=24)
    parser.add_argument("--seeds", default="7,19")
    parser.add_argument("--nodes", type=int, default=6)
    parser.add_argument("--families", default="0,1")
    args = parser.parse_args()
    seeds = [int(value) for value in args.seeds.split(",")]
    family_indices = [int(value) for value in args.families.split(",")]
    cases = [
        run_case(family_index, seed, args.nodes, args.iterations)
        for family_index in family_indices
        for seed in seeds
    ]
    ratios = np.asarray([case["fae_vs_best_control"] for case in cases])
    wins = sum(case["winner_objective"] == "fae" for case in cases)
    aggregate = {
        "cases": len(cases),
        "families": [FAMILIES[index].name for index in family_indices],
        "nodes": args.nodes,
        "seeds": seeds,
        "evaluations_per_arm_per_case": args.iterations,
        "fae_objective_wins": wins,
        "fae_win_rate": wins / len(cases),
        "median_ratio_vs_best_control": float(np.median(ratios)),
        "p90_ratio_vs_best_control": float(np.quantile(ratios, 0.9)),
        "gate": bool(wins / len(cases) >= 0.5 and np.median(ratios) < 0.95),
        "gate_rule": "FAE wins >=50% of cases on MSE+λ·edges·steps and median ratio < 0.95",
    }
    print(json.dumps({"aggregate": aggregate, "cases": cases}, indent=2))
    if not aggregate["gate"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
