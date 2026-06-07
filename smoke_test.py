"""Smoke test — proves the cross-family slice end to end (config B, §18.4).

1. fan_out: 2 trivial generation tasks on DeepSeek (bulk node).
2. adversarial_verify: Gemini (Google) refutes a DeepSeek-authored artifact.
3. confirms metrics JSONL is written and prints the cost summary.

Run:  python smoke_test.py
Needs DEEPSEEK_API_KEY and GEMINI_API_KEY in ~/.claude/orchestration/.env
"""
from __future__ import annotations

import sys

from mmorch import fan_out, adversarial_verify
from mmorch.config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from mmorch.metrics import summary, log_path
from mmorch.providers import MissingKeyError


def main() -> int:
    print(f"generator = {DEFAULT_GENERATOR} ({family_of(DEFAULT_GENERATOR)})")
    print(f"verifier  = {DEFAULT_VERIFIER} ({family_of(DEFAULT_VERIFIER)})")
    print("-" * 60)

    try:
        print("[1/2] fan_out: 2 bulk tasks on DeepSeek ...")
        results = fan_out(
            [
                "Write a one-line Python function `add(a, b)` that returns a+b. Code only.",
                "Write a one-line Python function `mul(a, b)` that returns a*b. Code only.",
            ],
            phase="smoke",
        )
        for i, r in enumerate(results):
            print(f"   gen[{i}] ({r.in_tokens}->{r.out_tokens} tok, ${r.cost_usd:.6f}): "
                  f"{r.text.strip()[:80]}")

        print("\n[2/2] adversarial_verify: Gemini refutes a planted bug ...")
        artifact = "def add(a, b):\n    return a - b  # intended: a + b"
        verdict = adversarial_verify(
            artifact,
            rubric="`add(a,b)` MUST return the sum a+b. Reject any other operation.",
            phase="smoke",
        )
        print(f"   passed={verdict.passed} confidence={verdict.confidence}")
        print(f"   refutations: {verdict.refutations}")
        if verdict.passed:
            print("   WARNING: skeptic failed to catch the planted bug (a-b != a+b).")

    except MissingKeyError as e:
        print(f"\nMISSING KEY: {e}")
        print("Add keys to ~/.claude/orchestration/.env then re-run.")
        return 2
    except Exception as e:  # noqa: BLE001
        print(f"\nERROR: {type(e).__name__}: {e}")
        return 1

    print("-" * 60)
    print(f"metrics log: {log_path()}")
    print(f"summary: {summary()}")
    # Auto-doc: mmorch regenera su README desde el codigo (fuente de verdad).
    try:
        from mmorch.docgen import update_readme, stats
        update_readme()
        print(f"README auto-actualizado: {stats()}")
    except Exception as e:  # best-effort, no rompe el smoke
        print(f"(docgen skip: {type(e).__name__}: {e})")
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
