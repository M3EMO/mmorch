"""code_review — cero-cupo senior reviewer: read code, flag where it breaks the mmorch coding
principles (docs/coding-principles.md), cross-family refuted so style-opinion noise gets pruned.

Reviewing principle-adherence is a SUBJECTIVE judgement (no executable ground truth), so the
generator→verifier pair MUST be cross-family (OneFlow): a model endorses its own blind spots.
The refuter drops weak/wrong/nitpick findings by default; it never invents authority the lens
doesn't grant. Two cheap external calls (find + refute), zero Claude cupo.

Library fn (`review`) with injectable `find`/`refute` so the self-check runs with no API.
"""
from __future__ import annotations

import json

from .config import DEFAULT_GENERATOR, DEFAULT_VERIFIER, family_of
from .providers import call
from .textutil import extract_fence

# Condensed from docs/coding-principles.md §"Review lens" — kept here so review() is self-contained.
# (Update both together if the lens changes; the doc is the human source of truth.)
LENS = """mmorch coding-principles review lens (apply in order):
1. Correctness/security: does it do what's asked? edge cases; parameterized queries, input
   validation at trust boundaries, isolate untrusted/LLM-generated code.
2. META (tie-breaker): does it minimize the next reader's cognitive load + next editor's change surface?
3. Modules: deep (much behavior, small interface) not shallow; high cohesion (one reason to change);
   low coupling (inject deps, no global state); locality (what changes together lives together).
4. Function/line: guard clauses over deep nesting; DRY (no copy-pasted behavior); meaningful names;
   minimal scope / least exposure (private by default).
5. Always: comments say WHY not how; robustness at boundaries; a silent `except: pass` only for a
   side-channel that must not break the main path (and commented); KISS — flag abstractions that
   don't pay for themselves (interface with one impl + no test seam = over-engineering)."""

_SEVERITY = {"high", "med", "medium", "low"}


def _parse(text: str) -> list[dict]:
    """Pull the JSON findings array out of a model reply (tolerant of prose/fences around it)."""
    blob = extract_fence(text)
    try:
        data = json.loads(blob)
    except Exception:
        i, j = blob.find("["), blob.rfind("]")
        if i < 0 or j < 0:
            return []
        try:
            data = json.loads(blob[i:j + 1])
        except Exception:
            return []
    if not isinstance(data, list):
        return []
    out = []
    for d in data:                      # keep only well-formed finding dicts
        if isinstance(d, dict) and d.get("problem"):
            out.append({"principle": str(d.get("principle", ""))[:80],
                        "severity": str(d.get("severity", "low")).lower(),
                        "line": d.get("line"),
                        "problem": str(d["problem"])[:400],
                        "fix": str(d.get("fix", ""))[:400]})
    return out


def _find(code: str, path: str, model: str) -> list[dict]:
    prompt = (f"{LENS}\n\nReview this code{f' ({path})' if path else ''}. List ONLY real violations "
              "of the lens. Return a JSON array of {principle, severity (high|med|low), line (int or "
              "null), problem, fix}. Empty array [] if it's clean. No prose outside the JSON.\n\n"
              f"```\n{code}\n```")
    return _parse(call(model, prompt, pattern="code_review", node="find").text)


def _refute(code: str, findings: list[dict], model: str) -> list[dict]:
    prompt = (f"{LENS}\n\nAnother reviewer flagged these violations:\n{json.dumps(findings, ensure_ascii=False)}\n\n"
              "You are a skeptical senior from a different background. For EACH, decide if it is a REAL "
              "violation of the lens — not a nitpick, not wrong, not a matter of taste. DROP it unless "
              "clearly real. Return the SURVIVING findings as the same JSON array (possibly empty).\n\n"
              f"```\n{code}\n```")
    return _parse(call(model, prompt, pattern="code_review", node="refute").text)


def review(code: str, *, path: str = "", gen_model: str = DEFAULT_GENERATOR,
           verifier_model: str = DEFAULT_VERIFIER, find=None, refute=None) -> dict:
    """Review `code` against the principles lens, cross-family refuted. Returns
    {path, findings:[...], n_raw, n_confirmed, dropped}. `find`/`refute` are injectable (test seam)."""
    if family_of(gen_model) == family_of(verifier_model):
        raise ValueError(f"subjective review needs cross-family: {gen_model} and {verifier_model} "
                         f"are both {family_of(gen_model)}")
    find = find or (lambda: _find(code, path, gen_model))
    refute = refute or (lambda fs: _refute(code, fs, verifier_model))
    raw = find()
    confirmed = refute(raw) if raw else []
    return {"path": path, "findings": confirmed, "n_raw": len(raw),
            "n_confirmed": len(confirmed), "dropped": len(raw) - len(confirmed)}


if __name__ == "__main__":
    # cero-cost self-check: injected find/refute, no API. Generator flags 2; refuter (skeptic) keeps 1.
    FINDINGS = [
        {"principle": "DRY", "severity": "med", "line": 10, "problem": "fence regex duplicated", "fix": "extract util"},
        {"principle": "naming", "severity": "low", "line": 3, "problem": "var named x", "fix": "rename"},
    ]
    r = review("def f(): pass", path="x.py",
               find=lambda: FINDINGS,
               refute=lambda fs: [fs[0]])          # skeptic drops the nitpick, keeps the DRY one
    assert r["n_raw"] == 2 and r["n_confirmed"] == 1 and r["dropped"] == 1, r
    assert r["findings"][0]["principle"] == "DRY", r
    # clean code -> refute not even called
    r2 = review("ok", find=lambda: [], refute=lambda fs: (_ for _ in ()).throw(AssertionError("must not run")))
    assert r2["n_confirmed"] == 0, r2
    # _parse tolerance: fenced JSON, bare array, junk
    assert len(_parse('```json\n[{"problem":"p"}]\n```')) == 1
    assert len(_parse('noise [{"problem":"p","severity":"HIGH"}] tail')) == 1
    assert _parse("no json here") == []
    # cross-family guard fires on same-family
    try:
        review("x", gen_model="deepseek-chat", verifier_model="deepseek-reasoner")
        assert False, "should reject same-family"
    except ValueError:
        pass
    print("code_review OK — find/refute seam, cross-family guard, parse tolerance")
