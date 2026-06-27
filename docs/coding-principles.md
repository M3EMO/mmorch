# Coding principles (mmorch standard)

The standard mmorch code is written and REVIEWED against. Condensed from a six-source synthesis,
ordered by leverage. The reviewer role checks against §"Review lens"; the architect/coder follow §1–4.

## The meta-principle (the tie-breaker)
**Minimize two quantities: the next reader's COGNITIVE LOAD and the next editor's CHANGE SURFACE.**
Cohesion, coupling, module depth, minimal scope, nesting, duplication, naming — all are ways to
measure or reduce one of those two. When two principles conflict, ask: *which option leaves less in
the head of whoever reads or changes this tomorrow?*

## 1. Modules (the highest-leverage level)
- **Deep modules**: much behavior behind a SMALL interface. Maximize behavior-per-unit-of-interface
  learned. `speedup(source,setup,call)`, `hillclimb(propose,score)`, `signature(task)` — tiny surface,
  deep impl. Shallow modules (big interface, little behind) are the thing to avoid.
- **Locality**: keep what changes together in ONE place. A bug/fix for a concern lives in one module.
- **Seams + injection = testability**: the real payoff of an interface is inserting a mock, NOT
  hypothetical hot-swapping. Inject deps (`gen=`, `bandit=`, `score=`, `path=`) so the seam is where
  the self-check substitutes a fake. Every non-trivial module leaves a runnable `__main__` self-check.

## 2. Cohesion & coupling
- **High cohesion**: a module serves ONE objective; everything in it is closely related (≈ SRP — one
  reason to change). SRP ≠ "do one thing"; it = "everything here changes for the same reason."
- **Low coupling**: components independent; one failing doesn't cascade. Pass parameters/inject; avoid
  global state (it breeds spaghetti + un-traceable bugs + race conditions under concurrency).
- **OCP by ADDITION**: extend by adding a module + wiring, not editing working code (the graft pattern:
  pure module → wire → self-check → one commit). The bandit takes any arm string; the intuition layer
  WRAPPED it instead of modifying it.

## 3. Functions & lines
- **No deep nesting**: guard clauses / early return collapse a level — once past a check, the reader
  drops it from memory. Extract a confusing condition to a named predicate.
- **DRY**: one behavior → one place. Duplicated logic = N places to change and one you'll miss.
  (See `textutil.extract_fence` — the fix for a fence helper that was copy-pasted 6×.)
- **Names**: follow SOME convention consistently; names meaningful to the next reader, not just you.
- **Minimal scope / least exposure**: declare data in the smallest scope; `private` by default. A
  "use locally only" comment does NOT hold — restrict access, don't request it.
- **Indirection in moderation**: extract when the piece has a clear name + single responsibility;
  do NOT extract a one-use fragment whose name says no more than its body. Inlining-everything and
  fragmenting-everything are both worse than the middle. Decide by module DEPTH, not line count.

## 4. Always (don't simplify these away)
- **Why-comments**: code says HOW, comments say WHY (the decision, the rationale, the ceiling). Mark
  deliberate shortcuts with the ceiling + upgrade path.
- **Robustness**: validate/clamp inputs at trust boundaries; fail-fast guards; manage resources;
  graceful degradation. A silent `except: pass` is allowed ONLY when a side-channel (telemetry/learning)
  must never break the main path — and then it carries a comment saying so. Don't let it hide real failures.
- **Security**: parameterized queries, sanitize inputs/encode outputs; never expose a service without
  auth; isolate untrusted/LLM-generated code in a subprocess + timeout; collect only needed data.
- **KISS over cleverness**: the simplest thing that works. An abstraction is justified ONLY if it
  reduces NET cognitive load (more leverage per interface) — else it's debt disguised as design.
  Introduce an interface when a SECOND concrete implementer exists (the test mock counts); not before.

## Honest tensions (judgment, not rules)
- **Extract vs over-indirection**: resolve by module depth — extract when it earns a name + a
  responsibility; don't when it just scatters a one-liner across files.
- **SRP-extreme vs deep modules**: SRP says what to GROUP (by reason-to-change); depth says how much to
  HIDE. Compatible only if you group things that change together behind a simple interface. SRP without
  locality = fragmentation into many shallow modules.
- **DIP/interfaces vs KISS**: an interface "in case we swap it" with one implementer is over-engineering;
  the defensible reason is testability. Add the abstraction when the 2nd implementer (incl. the mock) exists.
- **"More abstraction" is not a goal**: too little = tangled; too much = layers hiding what happens. Net
  cognitive load is the metric.

## Review lens (the reviewer applies these, in order)
1. Does it do what the task asked? Edge cases, correctness, security.
2. **Cognitive load / change surface** (the meta-principle) — the tie-breaker for everything below.
3. Module depth + locality; cohesion (one reason to change) + low coupling (injected, not global).
4. Nesting (guard clauses), duplication (DRY), names, minimal scope/exposure.
5. Why-comments present for non-obvious decisions; robustness at trust boundaries; no clever-for-its-own-sake.
6. KISS: is there a materially simpler version? Is any abstraction paying for itself?
Refute by default: if correctness/quality evidence isn't clear, REQUEST_CHANGES with the single
highest-priority fix.
