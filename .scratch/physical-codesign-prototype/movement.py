"""PROTOTYPE: search a relaxing network for less data movement, not more FLOPS."""

from __future__ import annotations

import argparse
import json

import numpy as np

from prototype import (
    Network,
    calibrate,
    edge_count,
    mse,
    mutate,
    random_network,
    raw_predictions,
)


# Relative proxies, not joules. e_byte >> e_op is the GPU fact under test.
E_OP = 1.0
E_BYTE = 50.0
E_ADC = 80.0
BYTES_PER_VALUE = 4.0
MSE_EDGE_PENALTY = 2e-5


def hops(steps: int, edges: int, batch: int) -> int:
    return int(steps) * int(edges) * int(batch)


def accounting(net: Network, inputs: np.ndarray, steps: int) -> dict:
    """Same prediction; three ways of paying for it."""
    batch = len(inputs)
    nodes = len(net.leak)
    edges = edge_count(net)
    traffic = hops(steps, edges, batch)
    adc_values = batch
    stationary = adc_values * BYTES_PER_VALUE * E_ADC
    streamed_g = steps * nodes * nodes * batch * BYTES_PER_VALUE * E_BYTE
    streamed_flops = steps * 2.0 * nodes * nodes * batch * E_OP
    return {
        "steps": int(steps),
        "edges": edges,
        "hops": traffic,
        "adc_values": adc_values,
        "stationary_proxy": float(stationary),
        "hbm_weight_proxy": float(streamed_g + streamed_flops),
    }


def score(train_mse: float, edges: int, steps: int, mode: str) -> float:
    if mode == "movement":
        return train_mse + MSE_EDGE_PENALTY * edges * steps
    return train_mse + MSE_EDGE_PENALTY * edges


def fit(net: Network, train_x: np.ndarray, train_y: np.ndarray) -> tuple[Network, float, int]:
    fitted, train_mse, physics = calibrate(net, train_x, train_y)
    return fitted, train_mse, int(physics["steps"])


def optimize_arm(
    initial: Network,
    *,
    mode: str,
    rng: np.random.Generator,
    iterations: int,
    train_x: np.ndarray,
    train_y: np.ndarray,
) -> tuple[Network, dict]:
    best, train_mse, steps = fit(initial, train_x, train_y)
    best_objective = score(train_mse, edge_count(best), steps, mode)
    accepted = 0
    for _ in range(iterations):
        proposal = mutate(best, rng, mode="co_design")
        candidate, candidate_mse, candidate_steps = fit(proposal, train_x, train_y)
        objective = score(
            candidate_mse,
            edge_count(candidate),
            candidate_steps,
            mode,
        )
        if objective < best_objective:
            best, best_objective = candidate, objective
            accepted += 1
    return best, {"accepted_mutations": accepted, "objective": best_objective}


def evaluate_net(
    net: Network,
    train_x: np.ndarray,
    train_y: np.ndarray,
    heldout_x: np.ndarray,
    heldout_y: np.ndarray,
) -> dict:
    _, physics = raw_predictions(net, heldout_x)
    costs = accounting(net, heldout_x, int(physics["steps"]))
    return {
        "train_mse": mse(net, train_x, train_y),
        "heldout_mse": mse(net, heldout_x, heldout_y),
        **costs,
    }


def run(seed: int, iterations: int) -> dict:
    rng = np.random.default_rng(seed)
    axis = np.linspace(-1.0, 1.0, 7)
    train_x = np.asarray([(x, y) for x in axis for y in axis])
    train_y = 0.8 * np.tanh(1.2 * train_x[:, 0] + 0.7 * train_x[:, 1])
    heldout_x = rng.uniform(-1.0, 1.0, size=(160, 2))
    heldout_y = 0.8 * np.tanh(1.2 * heldout_x[:, 0] + 0.7 * heldout_x[:, 1])

    initial, _, _ = fit(random_network(rng), train_x, train_y)
    initial_metrics = evaluate_net(initial, train_x, train_y, heldout_x, heldout_y)

    variants = {}
    for offset, mode in enumerate(("mse_only", "movement"), start=1):
        network, search = optimize_arm(
            initial,
            mode=mode,
            rng=np.random.default_rng(seed + offset * 1_000),
            iterations=iterations,
            train_x=train_x,
            train_y=train_y,
        )
        variants[mode] = {
            "evaluations": iterations,
            **search,
            **evaluate_net(network, train_x, train_y, heldout_x, heldout_y),
        }

    mse_arm = variants["mse_only"]
    move_arm = variants["movement"]
    gate = bool(
        move_arm["hops"] < mse_arm["hops"]
        and move_arm["heldout_mse"] <= 2.0 * mse_arm["heldout_mse"]
        and move_arm["heldout_mse"] < initial_metrics["heldout_mse"]
    )
    return {
        "seed": seed,
        "iterations": iterations,
        "cost_model": {
            "e_op": E_OP,
            "e_byte": E_BYTE,
            "e_adc": E_ADC,
            "bytes_per_value": BYTES_PER_VALUE,
            "note": "proxies; do not read as wall-plug joules",
        },
        "initial": initial_metrics,
        "arms": variants,
        "gate": gate,
        "gate_checks": {
            "movement_fewer_hops": move_arm["hops"] < mse_arm["hops"],
            "mse_within_2x": move_arm["heldout_mse"] <= 2.0 * mse_arm["heldout_mse"],
            "beats_initial_heldout": move_arm["heldout_mse"] < initial_metrics["heldout_mse"],
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=160)
    parser.add_argument("--no-repro-check", action="store_true")
    args = parser.parse_args()

    result = run(args.seed, args.iterations)
    if not args.no_repro_check:
        repeated = run(args.seed, args.iterations)
        if result != repeated:
            raise RuntimeError("same seed did not reproduce the same movement result")
    print(json.dumps(result, indent=2))
    if not result["gate"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
