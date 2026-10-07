"""PROTOTYPE gate: equal-budget co-design across tasks, sizes, seeds and OOD shifts."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from typing import Callable

import numpy as np

from prototype import calibrate, edge_count, mse, optimize, random_network


@dataclass(frozen=True)
class Family:
    name: str
    low: float
    train_high: float
    ood_high: float
    fn: Callable[[np.ndarray], np.ndarray]


FAMILIES = (
    Family(
        "tanh_mix",
        -0.7,
        0.7,
        1.2,
        lambda x: 0.8 * np.tanh(1.2 * x[:, 0] + 0.7 * x[:, 1]),
    ),
    Family(
        "nested_tanh",
        -0.7,
        0.7,
        1.2,
        lambda x: (
            0.5 * np.tanh(1.8 * x[:, 0])
            + 0.3 * np.tanh(0.8 * x[:, 1])
            + 0.1 * np.tanh(x[:, 0] + x[:, 1])
        ),
    ),
    Family(
        "rational_positive",
        0.0,
        0.7,
        1.2,
        lambda x: (1.4 * x[:, 0] + 0.8 * x[:, 1])
        / (1.0 + 0.9 * x[:, 0] + 0.5 * x[:, 1]),
    ),
)
MODES = ("co_design", "topology_only", "parameters_only", "random_search")


def datasets(family: Family, seed: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    axis = np.linspace(family.low, family.train_high, 7)
    train = np.asarray([(x, y) for x in axis for y in axis])
    rng = np.random.default_rng(seed)
    in_distribution = rng.uniform(family.low, family.train_high, size=(160, 2))
    pool = rng.uniform(family.low, family.ood_high, size=(640, 2))
    shifted = pool[np.any(pool > family.train_high, axis=1)]
    if family.low < 0:
        shifted = pool[np.any(np.abs(pool) > family.train_high, axis=1)]
    return train, in_distribution, shifted[:160]


def run_case(
    family: Family,
    *,
    nodes: int,
    seed: int,
    iterations: int,
) -> dict:
    train_x, id_x, ood_x = datasets(family, seed + nodes * 100)
    train_y, id_y, ood_y = family.fn(train_x), family.fn(id_x), family.fn(ood_x)
    initial, _, _ = calibrate(
        random_network(np.random.default_rng(seed), nodes=nodes),
        train_x,
        train_y,
    )
    variants = {}
    for offset, mode in enumerate(MODES, start=1):
        network, accepted = optimize(
            initial,
            mode=mode,
            rng=np.random.default_rng(seed + nodes * 10_000 + offset * 1_000),
            iterations=iterations,
            train_x=train_x,
            train_y=train_y,
        )
        variants[mode] = {
            "id_mse": mse(network, id_x, id_y),
            "ood_mse": mse(network, ood_x, ood_y),
            "edges": edge_count(network),
            "accepted": accepted,
        }
    winner = min(MODES, key=lambda mode: variants[mode]["ood_mse"])
    best_control = min(
        variants[mode]["ood_mse"] for mode in MODES if mode != "co_design"
    )
    return {
        "family": family.name,
        "nodes": nodes,
        "seed": seed,
        "evaluations_per_arm": iterations,
        "winner_ood": winner,
        "co_design_vs_best_control": variants["co_design"]["ood_mse"] / best_control,
        "variants": variants,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=200)
    parser.add_argument("--seeds", default="7,19,31")
    parser.add_argument("--nodes", default="6,10")
    args = parser.parse_args()
    seeds = [int(value) for value in args.seeds.split(",")]
    sizes = [int(value) for value in args.nodes.split(",")]

    cases = [
        run_case(family, nodes=nodes, seed=seed, iterations=args.iterations)
        for family in FAMILIES
        for nodes in sizes
        for seed in seeds
    ]
    ratios = np.asarray([case["co_design_vs_best_control"] for case in cases])
    wins = sum(case["winner_ood"] == "co_design" for case in cases)
    aggregate = {
        "cases": len(cases),
        "families": [family.name for family in FAMILIES],
        "sizes": sizes,
        "seeds": seeds,
        "evaluations_per_arm_per_case": args.iterations,
        "co_design_ood_wins": wins,
        "co_design_ood_win_rate": wins / len(cases),
        "median_ratio_vs_best_control": float(np.median(ratios)),
        "p90_ratio_vs_best_control": float(np.quantile(ratios, 0.9)),
        "gate": bool(wins / len(cases) >= 0.6 and np.median(ratios) < 0.9),
    }
    print(json.dumps({"aggregate": aggregate, "cases": cases}, indent=2))
    if not aggregate["gate"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
