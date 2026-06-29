"""signature — project a task's TEXT onto a STRUCTURAL key (cero-cupo, deterministic).

The intuition layer's only real net-new piece (see docs/intuition-layer.md). NOT a new
store/learner — its output is a `ctx` string fed into the EXISTING `feedback.contextual_arm`
+ ThompsonBandit, so a structurally-similar task reuses what worked, instead of the bandit
keying by raw task string (which never generalizes) or `recall` keying by surface embedding.

Deterministic keyword projection, NO LLM call on purpose:
  - cheap enough to compute at every routing decision (no API, no cupo),
  - frame-invariance is testable offline (re-describe -> same key?),
  - backfill projects 1000s of logged contexts with zero API spend.
Crude on purpose: a RICH set of cheap binary features (reservoir principle) — the bandit
weights them by measured outcome; we don't need a perfect minimal vocab up front.

ponytail: keyword projection is rung 1. If clustering on real data proves too coarse,
the upgrade path is an LLM-projected signature (one classify call) — NOT here, not yet.
"""
from __future__ import annotations

import re
from dataclasses import dataclass

# op_type: first match in PRIORITY order wins (specific ops before the GENERATE default).
# bilingual ES+EN — the feedback corpus is mostly Spanish task prompts.
_OPS = [
    ("REPAIR",    r"\b(fix|debug|repair|patch|arregl|corrig|repar|soluciona el (bug|error))\w*"),
    ("VERIFY",    r"\b(verif|valid|review|revis|chequea|check|ensure|comprob|assert|test\b|testea)\w*"),
    ("TRANSFORM", r"\b(refactor|translat|traduc|convert|conviert|rewrite|reescrib|optimiz|mejor|transform)\w*"),
    ("RANK",      r"\b(rank|rankea|compar|score|puntu|elegi el mejor|best of|ordena por)\w*"),
    ("SEARCH",    r"\b(find all|search|busca|encontra|localiza|grep|explora|investig)\w*"),
    ("DECIDE",    r"\b(decid|classif|clasif|route|rutea|should i|conviene|elegi (entre|cual))\w*"),
    ("GENERATE",  r"\b(generat|genera|write|escrib|implement|implementa|creat|crea|build|construi|resolve|resolv|solve|programa)\w*"),
]

# constraint_bits: rich, cheap, binary. Each = (bit_name, regex). Order irrelevant (it's a set).
_BITS = [
    ("exec_truth",      r"```|(\bdef )|assert\b|->|return |\bfunc\w*|devolv|pasa los|test"),
    ("code_artifact",   r"```|python|javascript|\bdef \b|\bfunc\w*|c[oó]digo|\bcode\b"),
    ("correctness_crit",r"exact|correct|estrict|exactamente|edge case|negativ|preciso|must\b|invariant"),
    ("multi_step",      r"\b(luego|then|primero|despu[eé]s|step|paso|stage|y donde|y adem[aá]s)\b"),
    ("ambiguous",       r"^\W*\w{0,18}\W*$|\balgo\b|\bbien\b\s*$|\?\s*$"),  # very short / vague / bare question
    ("exploration",     r"find all|todos los|explora|enumera|list all|map (the|el)"),
]

_GROUND = [
    ("needs_codebase",  r"\bthis (file|repo|function|module)\b|est[ae] (archivo|funci[oó]n|repo|m[oó]dulo)|el c[oó]digo|@\w+/"),
    ("needs_fresh",     r"\blatest\b|current\b|today|news|reciente|actual\b|202[4-9]|ultima versi"),
    ("needs_tools",     r"\b(run|execute|ejecuta|corre|fetch|descarga|api call|http)\b"),
]


@dataclass(frozen=True)
class Signature:
    op_type: str
    grounding: str
    bits: tuple[str, ...]            # sorted tuple of constraint_bit names (set, deterministic)
    complexity: str = ""            # optional Cynefin domain if the caller knows it (else "")

    def to_key(self) -> str:
        """Compact deterministic ctx string for feedback.contextual_arm(ctx=...)."""
        b = ",".join(self.bits) if self.bits else "none"
        c = f"|c={self.complexity}" if self.complexity else ""
        return f"{self.op_type}{c}|g={self.grounding}|b={b}"


def _first_match(text: str, table) -> str | None:
    for name, pat in table:
        if re.search(pat, text, re.I):
            return name
    return None


def signature(task: str, *, complexity: str = "") -> Signature:
    """Project a task prompt onto its structural Signature. Pure, deterministic, cero-cupo.
    `complexity` = optional Cynefin domain (clear|complicated|complex|chaotic) if the caller
    already knows it (e.g. workflow_obs rows carry it); left "" otherwise — the key stays stable."""
    t = task or ""
    op = _first_match(t, _OPS) or "GENERATE"
    ground = _first_match(t, _GROUND) or "self_contained"
    bits = tuple(sorted(name for name, pat in _BITS if re.search(pat, t, re.I)))
    return Signature(op_type=op, grounding=ground, bits=bits, complexity=complexity)


def key(task: str, *, complexity: str = "") -> str:
    """Convenience: signature(task).to_key()."""
    return signature(task, complexity=complexity).to_key()


if __name__ == "__main__":
    # 1. FRAME-INVARIANCE: re-describe the SAME task -> SAME op_type+grounding (the core claim).
    invariant_pairs = [
        ("Implementá en Python def inc(x): devolvé x+1",
         "Escribí la función inc(x) en Python que retorne x más uno"),
        ("Fix the bug in the auth middleware",
         "Arreglá el error en el middleware de autenticación"),
        ("Verificá que el output del generador es correcto",
         "Check that the generator's output is correct"),
    ]
    for a, b in invariant_pairs:
        sa, sb = signature(a), signature(b)
        assert sa.op_type == sb.op_type, f"op_type not invariant: {sa.op_type} vs {sb.op_type}\n  {a}\n  {b}"
    # 2. DISCRIMINATION: structurally different ops -> different op_type.
    assert signature("Fix this crash").op_type == "REPAIR"
    assert signature("Resolvé en Python: def f(a): ...").op_type == "GENERATE"
    assert signature("Refactor this function for readability").op_type == "TRANSFORM"
    assert signature("Verificá el resultado").op_type == "VERIFY"
    # 3. EXPECTED COLLISION (documented, NOT a bug): translate vs refactor both -> TRANSFORM.
    #    The key is coarse on purpose; recall returns BOTH strategies, execution-verify disposes.
    assert signature("Translate this Python to Java").op_type == "TRANSFORM"
    assert signature("Refactor this Python for speed").op_type == "TRANSFORM"
    # 4. bits + key are deterministic and stable across re-projection.
    s = signature("Resolvé en Python: def f(a): devolvé la suma. ```python```", complexity="complicated")
    assert "exec_truth" in s.bits and "code_artifact" in s.bits, s.bits
    assert s.to_key() == signature("Resolvé en Python: def f(a): devolvé la suma. ```python```",
                                   complexity="complicated").to_key()
    assert s.to_key().startswith("GENERATE|c=complicated|g="), s.to_key()
    print("signature OK — frame-invariance, discrimination, documented-collision, deterministic key")
    print("  example key:", s.to_key())
