# Sandbox executor — refined: NO hand-rolled Rust; harden + adopt the existing sandbox

Status: **DESIGN REVISED after a cross-family refine + reading the actual code.** The earlier draft
(a hand-rolled cross-platform Rust supervisor) is **REJECTED**. Two findings flipped it:

1. **mmorch already has `mmorch/sandbox.py`** doing exactly the recommended approach: separate process,
   **scrubbed env (no secrets/API keys)**, temp cwd, timeout+kill, a **static policy denylist**
   (blocks socket/subprocess/os.system/requests/file-writes pre-exec), and a **Docker backend**
   (`--network none --read-only --memory --pids-limit --cap-drop ALL`) = real isolation via an
   existing, battle-tested tool. The Rust design was redundant with this.
2. **Cross-family ensemble refuted the Rust design 0.9** (mmorch_ensemble_verify, cero cupo) on every axis.

## Why Rust was rejected (the refutations, all valid)
- **Reinvention / false security**: hand-rolling a cross-platform sandbox is hard and a *leaky* sandbox
  is worse than none. Use platform-native, vetted tools (Docker/nsjail/bubblewrap on Linux; Windows
  Sandbox/containers/Job-Objects on Windows). `sandbox.py` already shells Docker for the strong path.
- **"Portable Rust" is self-contradicting**: the OS primitives differ (seccomp/landlock vs Job-Objects/
  AppContainer) → it's two implementations anyway, not one portable binary. The portability claim collapses.
- **Windows-primary host vs Linux-first plan**: backwards; and robust untrusted isolation on Windows via
  a hand-rolled Rust binary is the hardest, least-vetted path.
- **Premature**: a hardened Python subprocess + an existing container gets ~80% now. The Rust binary's
  own "Phase-0 Python fallback" was a tacit admission a simpler path suffices.

→ Consistent with the language analysis: mmorch has **no module that justifies hand-rolled Rust.** The
sandbox was the last candidate, and it falls — the right tool is an *existing* sandbox, not new Rust.

## The real hole the refine surfaced (FIXED)
The threat model was complacent: LLM-generated code does non-malicious damage NOW — and concretely,
**`speedup.py` bypassed `sandbox.py`** and ran candidates with the **full inherited env**, so generated
code could read API keys / `MMORCH_SERVER_TOKEN` from `os.environ` and exfil them via stdout (captured +
logged). `checkers.py` correctly used `run_sandboxed`; speedup was the leak. **Fixed**: `speedup._measure`
now passes a scrubbed minimal env (verified: a generated `os.environ.get('MMORCH_SERVER_TOKEN')` → `<absent>`).

## Recommended posture (no new module, no Rust)
1. **One execution path.** All code-exec routes through `sandbox.py` (scrubbed env + temp cwd + timeout).
   speedup is now env-scrubbed; the DRY end-state is to route speedup's `_measure` *through*
   `run_sandboxed` — deferred only because `run_sandboxed` must first absorb speedup's Windows specifics
   (`CREATE_NO_WINDOW` + `stdin=DEVNULL`, which fixed a real MCP-stdio hang). Until then, two paths but
   both env-safe.
2. **Strong isolation = the Docker backend** (`backend="docker"`), already implemented: `--network none`,
   read-only fs + tmpfs, memory/pids limits, cap-drop ALL. Use it for genuinely untrusted code.
3. **Defense-in-depth = `enforce_policy=True`** (static denylist) on by default for untrusted runs, so
   dangerous code is rejected *before* execution, not relying on isolation alone.
4. **Windows strong-isolation gap** (when it matters): use **Windows Sandbox** or a container, invoked
   from Python — NOT a hand-rolled Rust Job-Objects supervisor.

## Build order (small, all Python, no Rust)
- **Done**: env-scrub `speedup._measure` (closes the exfil hole).
- **Next (cheap, real)**: port `CREATE_NO_WINDOW`/`stdin=DEVNULL` into `run_sandboxed`, then route
  `speedup._measure` through it → one safe execution path (DRY). Default `enforce_policy=True` for
  untrusted-code callers.
- **When threat rises**: make the Docker backend the default for untrusted runs; document Windows-Sandbox
  invocation for the Windows host.

## Non-goals
- No hand-rolled Rust sandbox (refuted: reinvention, false security, non-portable, premature).
- No new sandbox module — `sandbox.py` exists and is the right shape; harden/adopt it.

## Provenance
Cross-family refine: `mmorch_ensemble_verify` (DeepSeek gen, Gemini skeptics), unanimous refute 0.9,
$0.002 — killed the Rust design and exposed the threat-model complacency that led to finding the
speedup env-exfil hole.
