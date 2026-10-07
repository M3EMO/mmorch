"""PROTOTYPE: energy-relaxing nonlinear resistor network plus topology hillclimb."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass

import numpy as np


@dataclass
class Network:
    conductance: np.ndarray
    leak: np.ndarray
    alpha: np.ndarray
    scale: float = 1.0
    bias: float = 0.0

    def copy(self) -> "Network":
        return Network(
            self.conductance.copy(),
            self.leak.copy(),
            self.alpha.copy(),
            self.scale,
            self.bias,
        )


def energy(net: Network, voltage: np.ndarray) -> float:
    delta = voltage[:, None] - voltage[None, :]
    edge = 0.25 * np.sum(net.conductance * delta * delta)
    shunt = 0.5 * np.sum(net.leak * voltage**2)
    nonlinear = 0.25 * np.sum(net.alpha * voltage**4)
    return float(edge + shunt + nonlinear)


def energy_batch(net: Network, voltage: np.ndarray) -> np.ndarray:
    delta = voltage[:, :, None] - voltage[:, None, :]
    edge = 0.25 * np.sum(net.conductance[None, :, :] * delta * delta, axis=(1, 2))
    shunt = 0.5 * np.sum(net.leak[None, :] * voltage**2, axis=1)
    nonlinear = 0.25 * np.sum(net.alpha[None, :] * voltage**4, axis=1)
    return edge + shunt + nonlinear


def gradient(net: Network, voltage: np.ndarray) -> np.ndarray:
    laplacian = np.diag(net.conductance.sum(axis=1)) - net.conductance
    return laplacian @ voltage + net.leak * voltage + net.alpha * voltage**3


def relax(
    net: Network,
    inputs: np.ndarray,
    *,
    tolerance: float = 1e-8,
    max_steps: int = 4_000,
) -> tuple[np.ndarray, dict]:
    voltage = np.zeros(len(net.leak))
    voltage[:2] = inputs
    trace = [energy(net, voltage)]
    free = slice(2, None)
    residual = float("inf")
    for step in range(1, max_steps + 1):
        grad = gradient(net, voltage)
        residual = float(np.max(np.abs(grad[free])))
        if residual < tolerance:
            break
        degree_bound = 2.0 * float(net.conductance.sum(axis=1).max())
        local_curve = float(np.max(net.leak + 3.0 * net.alpha * voltage**2))
        dt = 0.8 / max(degree_bound + local_curve, 1e-9)
        voltage[free] -= dt * grad[free]
        trace.append(energy(net, voltage))
    else:
        raise RuntimeError(f"relaxation did not converge; residual={residual:.3g}")
    if any(after > before + 1e-11 for before, after in zip(trace, trace[1:])):
        raise RuntimeError("energy increased during passive relaxation")
    return voltage, {
        "steps": step,
        "residual": residual,
        "energy_initial": trace[0],
        "energy_final": trace[-1],
    }


def relax_batch(
    net: Network,
    inputs: np.ndarray,
    *,
    nudge_targets: np.ndarray | None = None,
    beta: float = 0.0,
    tolerance: float = 1e-8,
    max_steps: int = 100,
) -> tuple[np.ndarray, dict]:
    """Damped batched Newton solve; convex energy gives a unique equilibrium."""
    voltage = np.zeros((len(inputs), len(net.leak)))
    voltage[:, :2] = inputs
    laplacian = np.diag(net.conductance.sum(axis=1)) - net.conductance
    free_laplacian = laplacian[2:, 2:]
    def total_energy(value: np.ndarray) -> np.ndarray:
        result = energy_batch(net, value)
        if nudge_targets is not None and beta > 0:
            result = result + 0.5 * beta * (value[:, -1] - nudge_targets) ** 2
        return result

    previous_energy = total_energy(voltage)
    initial_energy = previous_energy.copy()
    residual = float("inf")
    for step in range(1, max_steps + 1):
        grad = voltage @ laplacian.T + net.leak * voltage + net.alpha * voltage**3
        if nudge_targets is not None and beta > 0:
            grad[:, -1] += beta * (voltage[:, -1] - nudge_targets)
        free_grad = grad[:, 2:]
        residual = float(np.max(np.abs(free_grad)))
        if residual < tolerance:
            break
        diagonal = net.leak[2:] + 3.0 * net.alpha[2:] * voltage[:, 2:] ** 2
        jacobian = np.broadcast_to(
            free_laplacian,
            (len(inputs), *free_laplacian.shape),
        ).copy()
        indices = np.arange(len(net.leak) - 2)
        jacobian[:, indices, indices] += diagonal
        if nudge_targets is not None and beta > 0:
            jacobian[:, -1, -1] += beta
        delta = np.linalg.solve(jacobian, free_grad[..., None])[..., 0]

        damping = 1.0
        for _ in range(24):
            trial = voltage.copy()
            trial[:, 2:] -= damping * delta
            trial_energy = total_energy(trial)
            if np.all(trial_energy <= previous_energy + 1e-12):
                voltage = trial
                previous_energy = trial_energy
                break
            damping *= 0.5
        else:
            raise RuntimeError("Newton line search could not lower physical energy")
    else:
        raise RuntimeError(f"batch relaxation did not converge; residual={residual:.3g}")
    return voltage, {
        "steps": step,
        "residual": residual,
        "energy_drop": float(np.max(initial_energy - previous_energy)),
    }


def raw_predictions(net: Network, inputs: np.ndarray) -> tuple[np.ndarray, dict]:
    voltage, state = relax_batch(net, inputs)
    return voltage[:, -1], state


def target(inputs: np.ndarray) -> np.ndarray:
    return 0.8 * np.tanh(1.2 * inputs[:, 0] + 0.7 * inputs[:, 1])


def calibrate(net: Network, inputs: np.ndarray, expected: np.ndarray) -> tuple[Network, float, dict]:
    raw, physics = raw_predictions(net, inputs)
    design = np.column_stack([raw, np.ones_like(raw)])
    scale, bias = np.linalg.lstsq(design, expected, rcond=None)[0]
    fitted = net.copy()
    fitted.scale = float(scale)
    fitted.bias = float(bias)
    prediction = fitted.scale * raw + fitted.bias
    return fitted, float(np.mean((prediction - expected) ** 2)), physics


def mse(net: Network, inputs: np.ndarray, expected: np.ndarray) -> float:
    raw, _ = raw_predictions(net, inputs)
    return float(np.mean((net.scale * raw + net.bias - expected) ** 2))


def random_network(rng: np.random.Generator, nodes: int = 8) -> Network:
    conductance = np.zeros((nodes, nodes))
    for i in range(nodes):
        for j in range(i + 1, nodes):
            if rng.random() < 0.38:
                conductance[i, j] = conductance[j, i] = rng.uniform(0.1, 1.4)
    # A minimal path prevents a degenerate output disconnected from both inputs.
    conductance[0, 2] = conductance[2, 0] = 0.7
    conductance[1, 3] = conductance[3, 1] = 0.7
    conductance[2, nodes - 1] = conductance[nodes - 1, 2] = 0.7
    conductance[3, nodes - 1] = conductance[nodes - 1, 3] = 0.7
    leak = rng.uniform(0.04, 0.2, size=nodes)
    alpha = rng.uniform(0.03, 0.4, size=nodes)
    leak[:2] = alpha[:2] = 0.0
    return Network(conductance, leak, alpha)


def mutate(net: Network, rng: np.random.Generator, mode: str = "co_design") -> Network:
    child = net.copy()
    if mode == "topology_only":
        move = int(rng.integers(2))
    elif mode == "parameters_only":
        move = int(rng.integers(2, 4))
    else:
        move = int(rng.integers(4))
    if move <= 1:
        i, j = sorted(rng.choice(len(child.leak), size=2, replace=False))
        old = child.conductance[i, j]
        if old > 0 and rng.random() < 0.18:
            value = 0.0
        else:
            value = float(np.clip((old or 0.2) * np.exp(rng.normal(0, 0.35)), 0.03, 3.0))
        child.conductance[i, j] = child.conductance[j, i] = value
    elif move == 2:
        i = int(rng.integers(2, len(child.leak)))
        child.alpha[i] = float(np.clip(child.alpha[i] * np.exp(rng.normal(0, 0.3)), 0.005, 2.0))
    else:
        i = int(rng.integers(2, len(child.leak)))
        child.leak[i] = float(np.clip(child.leak[i] * np.exp(rng.normal(0, 0.3)), 0.01, 1.0))
    return child


def edge_count(net: Network) -> int:
    return int(np.count_nonzero(np.triu(net.conductance, 1)))


def optimize(
    initial: Network,
    *,
    mode: str,
    rng: np.random.Generator,
    iterations: int,
    train_x: np.ndarray,
    train_y: np.ndarray,
) -> tuple[Network, int]:
    best, train_mse, _ = calibrate(initial, train_x, train_y)
    best_objective = train_mse + 2e-5 * edge_count(best)
    accepted = 0
    for _ in range(iterations):
        proposal = (
            random_network(rng, nodes=len(initial.leak))
            if mode == "random_search"
            else mutate(best, rng, mode=mode)
        )
        candidate, candidate_mse, _ = calibrate(proposal, train_x, train_y)
        objective = candidate_mse + 2e-5 * edge_count(candidate)
        if objective < best_objective:
            best, best_objective = candidate, objective
            accepted += 1
    return best, accepted


def search(seed: int, iterations: int) -> tuple[Network, dict]:
    rng = np.random.default_rng(seed)
    axis = np.linspace(-1.0, 1.0, 7)
    train_x = np.asarray([(x, y) for x in axis for y in axis])
    train_y = target(train_x)
    heldout_x = rng.uniform(-1.0, 1.0, size=(160, 2))
    heldout_y = target(heldout_x)

    initial, initial_mse, initial_physics = calibrate(random_network(rng), train_x, train_y)
    modes = ("co_design", "topology_only", "parameters_only", "random_search")
    variants = {}
    for offset, mode in enumerate(modes, start=1):
        candidate, accepted = optimize(
            initial,
            mode=mode,
            rng=np.random.default_rng(seed + offset * 1_000),
            iterations=iterations,
            train_x=train_x,
            train_y=train_y,
        )
        variants[mode] = {
            "network": candidate,
            "accepted_mutations": accepted,
            "train_mse": mse(candidate, train_x, train_y),
            "heldout_mse": mse(candidate, heldout_x, heldout_y),
            "edges": edge_count(candidate),
        }
    best = variants["co_design"]["network"]
    accepted = variants["co_design"]["accepted_mutations"]

    final_train = mse(best, train_x, train_y)
    initial_heldout = mse(initial, heldout_x, heldout_y)
    final_heldout = mse(best, heldout_x, heldout_y)
    _, final_physics = raw_predictions(best, heldout_x[:32])

    robust = []
    perturb_rng = np.random.default_rng(seed + 10_000)
    for _ in range(16):
        perturbed = best.copy()
        noise = perturb_rng.normal(0.0, 0.05, size=perturbed.conductance.shape)
        noise = (noise + noise.T) / 2
        perturbed.conductance *= np.clip(1.0 + noise, 0.0, None)
        np.fill_diagonal(perturbed.conductance, 0.0)
        robust.append(mse(perturbed, heldout_x[:48], heldout_y[:48]))

    summary = {
        "seed": seed,
        "iterations": iterations,
        "accepted_mutations": accepted,
        "initial": {
            "train_mse": initial_mse,
            "heldout_mse": initial_heldout,
            "edges": edge_count(initial),
            "physics": initial_physics,
        },
        "equal_budget_controls": {
            mode: {
                "evaluations": iterations,
                "accepted_mutations": values["accepted_mutations"],
                "train_mse": values["train_mse"],
                "heldout_mse": values["heldout_mse"],
                "edges": values["edges"],
            }
            for mode, values in variants.items()
        },
        "winner_by_heldout": min(modes, key=lambda mode: variants[mode]["heldout_mse"]),
        "final": {
            "train_mse": final_train,
            "heldout_mse": final_heldout,
            "edges": edge_count(best),
            "scale": best.scale,
            "bias": best.bias,
            "physics": final_physics,
            "robust_mse_mean": float(np.mean(robust)),
            "robust_mse_p95": float(np.quantile(robust, 0.95)),
        },
        "conductance": np.round(best.conductance, 4).tolist(),
        "leak": np.round(best.leak, 4).tolist(),
        "alpha": np.round(best.alpha, 4).tolist(),
    }
    return best, summary


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=320)
    parser.add_argument("--no-repro-check", action="store_true")
    args = parser.parse_args()

    _, result = search(args.seed, args.iterations)
    if not args.no_repro_check:
        _, repeated = search(args.seed, args.iterations)
        if result != repeated:
            raise RuntimeError("same seed did not reproduce the same result")
    if result["final"]["heldout_mse"] >= result["initial"]["heldout_mse"]:
        raise RuntimeError("topology search did not improve held-out MSE")
    if result["final"]["robust_mse_p95"] >= result["initial"]["heldout_mse"]:
        raise RuntimeError("improvement did not survive 5% component perturbations")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
