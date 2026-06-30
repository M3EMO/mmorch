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
from .signature import Signature, key as sig_key, signature

ROOT = Path(__file__).resolve().parent.parent
_SIG_BANDIT = ROOT / "logs" / "bandit_sig.json"


def _arm(model: str, task: str, complexity: str = "") -> str:
    # key by MODEL only: strip any "@thr" suffix (threshold is cascade's decision, not this
    # bandit's) so a model's samples pool across thresholds and the arm is a valid model key.
    return f"{model.split('@')[0]}#{sig_key(task, complexity=complexity)}"


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
    out = []
    for m in models:
        s = stats.get(_arm(m, task, complexity=complexity), {"mean": 0.5, "n": 0})
        out.append((m.split("@")[0], s["mean"], s["n"]))
    out.sort(key=lambda t: (-t[1], -t[2]))
    return out[:k]


def coherence(task: str, *, complexity: str = "", bandit: ThompsonBandit | None = None) -> int:
    """Familiarity at this signature = total samples seen across all its arms. Gate input:
    high coherence -> commit fast; low -> escalate/explore."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    sk = sig_key(task, complexity=complexity)
    return sum(s["n"] for a, s in b.stats().items() if a.endswith("#" + sk))


def decide(models: list[str], task: str, *, complexity: str = "", threshold: float = 0.62,
           min_n: int = 5, bandit: ThompsonBandit | None = None) -> tuple[str, str | None, str]:
    """Phase 3 GATE (one-line policy, hysteresis deferred): if this signature is FAMILIAR
    (coherence >= min_n) AND its best candidate is good enough (mean >= threshold, n >= min_n),
    COMMIT to that model cheaply — no escalation. Else ESCALATE (let route/Opus decide).
    Returns (action, model|None, reason)."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    coh = coherence(task, complexity=complexity, bandit=b)
    best_model, best_mean, best_n = candidates(models, task, complexity=complexity, k=1, bandit=b)[0]
    if coh >= min_n and best_mean >= threshold and best_n >= min_n:
        return ("commit", best_model, f"familiar coh={coh} best={best_model}@{best_mean:.2f}(n={best_n})")
    return ("escalate", None, f"unfamiliar/weak coh={coh} best={best_mean:.2f}(n={best_n})")


def reframe(task: str, *, complexity: str = "") -> list[str]:
    """Phase 4 INSIGHT: on impasse, RE-REPRESENT by relaxing the signature one step at a time
    -> broader NEIGHBOR signature keys (most-specific relaxation first). Dropping a constraint
    bit recalls the more general bucket; dropping grounding/complexity is the coarse fallback.
    (This is also the cold-start neighbor-pooling — predictive-coding's 'predict from above'.)"""
    s = signature(task, complexity=complexity)
    out: list[str] = []
    for drop in s.bits:  # relax each constraint bit -> a more general signature
        out.append(Signature(s.op_type, s.grounding, tuple(x for x in s.bits if x != drop), s.complexity).to_key())
    if s.complexity:     # drop the Cynefin level
        out.append(Signature(s.op_type, s.grounding, s.bits, "").to_key())
    out.append(Signature(s.op_type, "self_contained", (), "").to_key())  # op_type-only coarse fallback
    seen, uniq = set(), []
    for k in out:
        if k not in seen:
            seen.add(k); uniq.append(k)
    return uniq


def candidates_pooled(models: list[str], task: str, *, complexity: str = "", k: int = 3,
                      bandit: ThompsonBandit | None = None) -> list[tuple[str, float, int]]:
    """INSIGHT-backed recall: if the exact signature is COLD (all n==0), fall back to the
    nearest NEIGHBOR signature (reframe) that has evidence, and return ITS candidate set.
    Cold tasks inherit what worked on structurally-adjacent tasks instead of pure exploration."""
    b = bandit or ThompsonBandit(_SIG_BANDIT)
    direct = candidates(models, task, complexity=complexity, k=k, bandit=b)
    if any(n > 0 for _, _, n in direct):
        return direct
    stats = b.stats()
    for sk in reframe(task, complexity=complexity):
        pooled = []
        for m in models:
            s = stats.get(f"{m.split('@')[0]}#{sk}", {"mean": 0.5, "n": 0})
            pooled.append((m.split("@")[0], s["mean"], s["n"]))
        if any(n > 0 for _, _, n in pooled):
            pooled.sort(key=lambda t: (-t[1], -t[2]))
            return pooled[:k]
    return direct  # genuinely novel — nothing adjacent; caller explores/escalates


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
    # Phase 3 GATE: familiar+good -> commit; cold -> escalate.
    act, mdl, _r = decide(["deepseek-chat", "gemini-2.5-flash"], code_task, bandit=bb)
    assert act == "commit" and mdl == "deepseek-chat", (act, mdl)
    act2, mdl2, _r = decide(["deepseek-chat", "gemini-2.5-flash"], other_task, bandit=bb)
    assert act2 == "escalate" and mdl2 is None, (act2, mdl2)
    # Phase 4 INSIGHT: a cold task that's structurally adjacent inherits the neighbor's evidence.
    warm = "Generá en Python def f(a): devolvé la suma"
    for _ in range(6):
        record("glm-4.6", 1.0, warm, bandit=bb)
    cold_variant = "Generá en Python def f(a): devolvé la suma EXACTA con negativos estrictamente"
    assert all(n == 0 for _, _, n in candidates(["glm-4.6", "x"], cold_variant, bandit=bb)), "exact sig should be cold"
    pooled = candidates_pooled(["glm-4.6", "x"], cold_variant, bandit=bb)
    assert pooled[0][0] == "glm-4.6" and pooled[0][2] > 0, ("insight pooling should find the neighbor", pooled)
    assert reframe(cold_variant), "reframe should yield neighbor signatures"
    # @thr is stripped: an outcome recorded with "model@0.0" is found by the bare "model".
    record("deepseek-v4-pro@0.0", 1.0, code_task, bandit=bb)
    assert any(m == "deepseek-v4-pro" and n > 0
               for m, _, n in candidates(["deepseek-v4-pro"], code_task, bandit=bb)), "thr-strip keying"
    tmp.unlink()
    print("intuition OK — sig-keyed select, candidate set, coherence, gate(commit/escalate), insight pooling")
