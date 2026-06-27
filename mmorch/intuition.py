"""intuition — the bandit, re-keyed by structural signature (intuition layer Phase 1).

NOT a new store/learner: it wraps the EXISTING feedback.ThompsonBandit (Beta posteriors,
Thompson sampling, no catastrophic forgetting) and just keys arms by the structural
`signature` instead of the raw task string. A structurally-similar task therefore reuses
what worked, instead of every task being a fresh cold arm.

arm key = f"{model_arm}#{signature_key}"  (e.g. "deepseek-chat@0.6#GENERATE|g=self...|b=exec_truth")

Separate state file (bandit_sig.json) so it never collides with the flat bandit_state.json.
- record(model, reward, task): learn from one outcome.
- select(models, task): Thompson-pick the best model for this signature (cold sigs explore).
- candidates(models, task): the top-K SET (recall is high-recall; VERIFY disposes — never the key).
- coherence(task): familiarity = total samples seen at this signature (the gate's input).
- backfill(): one-time replay of the existing logs into the sig-bandit.

ponytail: forward-wiring into live routing = Phase 5 (touches the routers). Here we only need
the bandit to HOLD real per-signature posteriors, which backfill() gives it cero-cupo.
"""
from __future__ import annotations

import json
from pathlib import Path

from .feedback import ThompsonBandit
from .signature import key as sig_key

ROOT = Path(__file__).resolve().parent.parent
_SIG_BANDIT = ROOT / "logs" / "bandit_sig.json"


def _arm(model: str, task: str, complexity: str = "") -> str:
    return f"{model}#{sig_key(task, complexity=complexity)}"


def record(model: str, reward: float, task: str, *, complexity: str = "",
           bandit: ThompsonBandit | None = None) -> str:
    """Learn from one outcome at this task's signature. Returns the sig-keyed arm."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    arm = _arm(model, task, complexity)
    b.update(arm, reward)
    return arm


def select(models: list[str], task: str, *, complexity: str = "",
           bandit: ThompsonBandit | None = None) -> str:
    """Thompson-pick the best MODEL for this signature. Cold signatures explore (Beta(1,1))."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    arms = [_arm(m, task, complexity) for m in models]
    chosen = b.select(arms)
    return chosen.split("#", 1)[0]


def candidates(models: list[str], task: str, *, complexity: str = "", k: int = 3,
               bandit: ThompsonBandit | None = None) -> list[tuple[str, float, int]]:
    """The candidate SET: top-k (model, posterior_mean, n) by mean for this signature.
    High-recall on purpose — precision comes from VERIFY, never the key."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    stats = b.stats()
    sk = sig_key(task, complexity=complexity)
    out = []
    for m in models:
        s = stats.get(f"{m}#{sk}", {"mean": 0.5, "n": 0})
        out.append((m, s["mean"], s["n"]))
    out.sort(key=lambda t: (-t[1], -t[2]))
    return out[:k]


def coherence(task: str, *, complexity: str = "", bandit: ThompsonBandit | None = None) -> int:
    """Familiarity at this signature = total samples seen across all its arms. Gate input:
    high coherence -> commit fast; low -> escalate/explore."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    sk = sig_key(task, complexity=complexity)
    return sum(s["n"] for a, s in b.stats().items() if a.endswith("#" + sk))


def _load(p: Path) -> list[dict]:
    return [json.loads(l) for l in p.read_text(encoding="utf-8").splitlines() if l.strip()] if p.exists() else []


def backfill(*, reset: bool = True, bandit_path: Path = _SIG_BANDIT) -> dict:
    """One-time replay of the existing logs into the sig-bandit. reset=True rebuilds from
    scratch (idempotent — safe to re-run). Returns a stats report."""
    if reset and bandit_path.exists():
        bandit_path.unlink()
    b = ThompsonBandit(bandit_path)
    sources = {
        "feedback": (ROOT / "logs" / "feedback.jsonl", "arm", "context", None),
        "trajectories": (ROOT / "logs" / "trajectories.jsonl", "gen_model", "task", None),
        "workflow_obs": (ROOT / "logs" / "workflow_obs.jsonl", None, "task", "domain"),
    }
    rep: dict = {"by_source": {}, "total_updates": 0}
    for name, (path, armf, taskf, cplxf) in sources.items():
        rows = _load(path)
        n = 0
        rewards = []
        for r in rows:
            task = (r.get(taskf) or "").strip()
            rew = r.get("reward")
            if not task or not isinstance(rew, (int, float)):
                continue
            model = (r.get(armf) if armf else None) or r.get("gen_model") or r.get("arm") or "unknown"
            cplx = (r.get(cplxf) or "") if cplxf else ""
            record(model, float(rew), task, complexity=cplx, bandit=b)
            n += 1
            rewards.append(float(rew))
        rep["by_source"][name] = {
            "rows": len(rows), "used": n,
            "reward_mean": round(sum(rewards) / len(rewards), 3) if rewards else None,
            "reward_variance": round(_var(rewards), 3) if len(rewards) > 1 else 0.0,
        }
        rep["total_updates"] += n
    stats = b.stats()
    rep["distinct_arms"] = len(stats)
    sigs = {a.split("#", 1)[1] for a in stats if "#" in a}
    rep["distinct_signatures"] = len(sigs)
    return rep


def _var(xs: list[float]) -> float:
    if len(xs) < 2:
        return 0.0
    m = sum(xs) / len(xs)
    return sum((x - m) ** 2 for x in xs) / len(xs)


if __name__ == "__main__":
    # isolated self-check (temp bandit, no real logs touched)
    import tempfile
    tmp = Path(tempfile.gettempdir()) / "bandit_sig_selfcheck.json"
    if tmp.exists():
        tmp.unlink()
    bb = ThompsonBandit(tmp)
    code_task = "Resolvé en Python: def f(a): devolvé la suma ```python```"
    other_task = "Fix the crash in the auth handler"
    # deepseek wins on the code signature; gemini loses there
    for _ in range(8):
        record("deepseek-chat", 1.0, code_task, bandit=bb)
        record("gemini-2.5-flash", 0.0, code_task, bandit=bb)
    pick = select(["deepseek-chat", "gemini-2.5-flash"], code_task, bandit=bb)
    assert pick == "deepseek-chat", f"sig bandit should prefer deepseek on code sig, got {pick}"
    cands = candidates(["deepseek-chat", "gemini-2.5-flash"], code_task, bandit=bb)
    assert cands[0][0] == "deepseek-chat" and cands[0][1] > cands[1][1], cands
    # coherence: code sig is familiar (16 samples), the other sig is cold (0)
    assert coherence(code_task, bandit=bb) == 16, coherence(code_task, bandit=bb)
    assert coherence(other_task, bandit=bb) == 0, coherence(other_task, bandit=bb)
    tmp.unlink()
    print("intuition OK — sig-keyed select, candidate set, coherence (familiar vs cold)")
