# mmorch Session Learning (v0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let mmorch learn from Claude Code session transcripts by extracting real outcomes and calibrating the Cynefin router against observed task difficulty.

**Architecture:** A new `mmorch/sessions.py` parses a session JSONL into task segments, derives a deterministic outcome label (external signal only), estimates observed difficulty → an observed Cynefin domain, and feeds `feedback.record_outcome` to calibrate `cynefin_classify`. Redaction is built and tested for the deferred external-enrichment slice; v0 itself makes no external API call (fully local, zero leak, zero cost).

**Tech Stack:** Python 3, stdlib (json, re, hashlib, pathlib, dataclasses), existing mmorch modules (`feedback`, `classify`). pytest for tests. No new dependencies.

## Global Constraints

- Label comes from EXTERNAL signal only (user words or deterministic tool output). NEVER from Claude's own "done/listo" — anti-sycophancy / cobra effect. Copied from spec invariant 1 & 2.
- Outcome is deterministic, never an LLM judge of success. Spec invariant 2.
- No raw transcript ever leaves the machine to an external API. Spec no-objetivo. v0 makes no external call at all.
- Abstain over dirty data: no clear signal → record nothing. Spec invariant 5.
- Idempotent: a session is ingested at most once (tracked by content hash). Spec invariant 4.
- Library-only modules; MCP wrapper is the only I/O surface. Follows existing mmorch pattern (`classify.py` + `mcp_server.py`).
- Source tag on every recorded outcome: `source="claude_session"`.

---

### Task 1: Session parser — JSONL → Segments

**Files:**
- Create: `mmorch/sessions.py`
- Test: `tests/test_sessions_parse.py`

**Interfaces:**
- Produces: `@dataclass Segment(request: str, reasoning: str, tool_calls: list[dict], tool_results: list[dict])` where `tool_calls` items are `{"name": str, "input": dict}` and `tool_results` items are `{"content": str, "is_error": bool}`. `parse_session(path: str | Path) -> list[Segment]`.

A Claude session JSONL has one event per line with a top-level `type`. Relevant types: `user` (`message.content` is a **str** = a real prompt, OR a **list** containing `tool_result` blocks), `assistant` (`message.content` is a list of `text` and `tool_use` blocks). A new Segment starts at each `user` event whose content is a str.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_parse.py
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def _write_jsonl(tmp_path, events):
    p = tmp_path / "sess.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in events), encoding="utf-8")
    return p


def test_parse_splits_segments_and_extracts_blocks(tmp_path):
    events = [
        {"type": "user", "message": {"role": "user", "content": "arregla el bug"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "miro el archivo"},
            {"type": "tool_use", "name": "Edit", "input": {"file_path": "a.py"}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "ok", "is_error": False}]}},
        {"type": "user", "message": {"role": "user", "content": "ahora corre tests"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "input": {"command": "pytest"}}]}},
    ]
    segs = S.parse_session(_write_jsonl(tmp_path, events))
    assert len(segs) == 2
    assert segs[0].request == "arregla el bug"
    assert segs[0].reasoning == "miro el archivo"
    assert segs[0].tool_calls == [{"name": "Edit", "input": {"file_path": "a.py"}}]
    assert segs[0].tool_results == [{"content": "ok", "is_error": False}]
    assert segs[1].request == "ahora corre tests"
    assert segs[1].tool_calls[0]["name"] == "Bash"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_parse.py -q`
Expected: FAIL with `ModuleNotFoundError: No module named 'mmorch.sessions'`

- [ ] **Step 3: Write minimal implementation**

```python
# mmorch/sessions.py
"""sessions — aprende de transcripts de Claude Code. Parsea el JSONL de sesion en
segmentos de tarea, deriva un outcome determinista (label externo), estima la
dificultad observada y calibra cynefin_classify via feedback.record_outcome.
v0: 100% local, sin API externa, sin fuga. Library-only."""
from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Segment:
    request: str
    reasoning: str = ""
    tool_calls: list[dict] = field(default_factory=list)     # {name, input}
    tool_results: list[dict] = field(default_factory=list)   # {content, is_error}


def _content_blocks(msg: dict) -> list | str:
    c = msg.get("content") if isinstance(msg, dict) else None
    return c if c is not None else []


def parse_session(path: str | Path) -> list[Segment]:
    """JSONL -> segmentos. Un segmento arranca en cada user-prompt (content str)."""
    segments: list[Segment] = []
    cur: Segment | None = None
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        ev = json.loads(line)
        t, msg = ev.get("type"), ev.get("message", {})
        if t == "user":
            content = _content_blocks(msg)
            if isinstance(content, str):                 # prompt real -> nuevo segmento
                cur = Segment(request=content)
                segments.append(cur)
            elif isinstance(content, list) and cur is not None:
                for b in content:
                    if isinstance(b, dict) and b.get("type") == "tool_result":
                        res = b.get("content", "")
                        if isinstance(res, list):        # bloques -> texto plano
                            res = " ".join(x.get("text", "") for x in res if isinstance(x, dict))
                        cur.tool_results.append(
                            {"content": str(res), "is_error": bool(b.get("is_error", False))})
        elif t == "assistant" and cur is not None:
            for b in _content_blocks(msg):
                if not isinstance(b, dict):
                    continue
                if b.get("type") == "text":
                    cur.reasoning = (cur.reasoning + " " + b.get("text", "")).strip()
                elif b.get("type") == "tool_use":
                    cur.tool_calls.append({"name": b.get("name", ""), "input": b.get("input", {})})
    return segments
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_parse.py -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add mmorch/sessions.py tests/test_sessions_parse.py
git commit -m "feat(sessions): parse Claude session JSONL into task segments"
```

---

### Task 2: Deterministic outcome label

**Files:**
- Modify: `mmorch/sessions.py`
- Test: `tests/test_sessions_outcome.py`

**Interfaces:**
- Consumes: `Segment` from Task 1.
- Produces: `@dataclass Outcome(reward: float, source: str, confidence: float)`. `outcome_of(seg: Segment, next_request: str = "") -> Outcome | None`. Returns `None` when no external signal exists (abstain). `next_request` is the FOLLOWING segment's user prompt — the user's reaction to this segment.

Positive: a tool_result shows tests passed (`"passed"` and not `"failed"` in a pytest-like result) or a successful commit; or `next_request` contains an acceptance word. Negative: a revert/undo tool call, an acceptance-negative word in `next_request`, or an unresolved error tool_result with no recovery.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_outcome.py
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")
Seg = S.Segment


def test_tests_passed_is_positive():
    seg = Seg(request="corre tests", tool_results=[{"content": "5 passed in 2s", "is_error": False}])
    o = S.outcome_of(seg)
    assert o is not None and o.reward >= 0.8 and o.source == "tool"


def test_user_acceptance_is_positive():
    seg = Seg(request="hace X")
    o = S.outcome_of(seg, next_request="perfecto, funciona")
    assert o is not None and o.reward >= 0.8 and o.source == "user"


def test_user_rejection_is_negative():
    seg = Seg(request="hace X")
    o = S.outcome_of(seg, next_request="no, esta mal, rehacelo")
    assert o is not None and o.reward <= 0.2


def test_tests_failed_is_negative():
    seg = Seg(request="corre tests", tool_results=[{"content": "1 failed, 4 passed", "is_error": False}])
    o = S.outcome_of(seg)
    assert o is not None and o.reward <= 0.2


def test_no_signal_abstains():
    seg = Seg(request="contame algo", reasoning="bla")
    assert S.outcome_of(seg) is None


def test_claude_self_report_is_not_a_label():
    # anti-sicofancia: el "listo!" del propio assistant NO cuenta como label.
    seg = Seg(request="hace X", reasoning="listo, funciona perfecto, done")
    assert S.outcome_of(seg) is None
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_outcome.py -q`
Expected: FAIL with `AttributeError: module 'mmorch.sessions' has no attribute 'outcome_of'`

- [ ] **Step 3: Write minimal implementation**

Append to `mmorch/sessions.py`:

```python
_ACCEPT = ("funciona", "perfecto", "gracias", "dale", "joya", "excelente", "buenisimo", "anda")
_REJECT = ("no,", "mal", "rehace", "rehacer", "no funciona", "error", "esta roto", "revert")
_REVERT_TOOLS = ("revert", "undo", "checkout --")


@dataclass
class Outcome:
    reward: float
    source: str        # "tool" | "user"
    confidence: float = 1.0


def _tests_signal(seg: "Segment") -> float | None:
    for r in seg.tool_results:
        c = r["content"].lower()
        if "passed" in c or "failed" in c:
            failed = ("failed" in c or "error" in c) and "0 failed" not in c
            return 0.0 if failed else 1.0
    return None


def outcome_of(seg: "Segment", next_request: str = "") -> "Outcome | None":
    """Label determinista de SEÑAL EXTERNA (tool o user). None si no hay señal.
    Nunca usa el texto del propio assistant (anti-sicofancia)."""
    t = _tests_signal(seg)
    if t is not None:
        return Outcome(reward=t, source="tool")
    for tc in seg.tool_calls:                       # revert explicito = negativo
        cmd = str(tc.get("input", {}).get("command", "")).lower()
        if any(k in cmd for k in _REVERT_TOOLS):
            return Outcome(reward=0.0, source="tool")
    nx = next_request.lower().strip()
    if nx:
        if any(w in nx for w in _REJECT):
            return Outcome(reward=0.0, source="user")
        if any(w in nx for w in _ACCEPT):
            return Outcome(reward=1.0, source="user")
    return None
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_outcome.py -q`
Expected: PASS (6 passed)

- [ ] **Step 5: Commit**

```bash
git add mmorch/sessions.py tests/test_sessions_outcome.py
git commit -m "feat(sessions): deterministic external-signal outcome label"
```

---

### Task 3: Secret redaction (privacy gate for deferred enrichment)

**Files:**
- Modify: `mmorch/sessions.py`
- Test: `tests/test_sessions_redact.py`

**Interfaces:**
- Produces: `redact(text: str) -> tuple[str, float]`. Returns `(redacted_text, confidence)`. Strips API keys, KEY=VALUE secrets, emails, home paths. `confidence` is 0.0 if a residual high-entropy token (≥32 chars of base64/hex not already redacted) remains, else 1.0. Built now so the deferred external-enrichment slice has a tested gate; v0 makes no external call.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_redact.py
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def test_redacts_api_key():
    red, _ = S.redact("my key is sk-ABCDEF0123456789abcdef0123 ok")
    assert "sk-ABCDEF0123456789abcdef0123" not in red and "[REDACTED]" in red


def test_redacts_key_value_secret():
    red, _ = S.redact("API_KEY=supersecretvalue123")
    assert "supersecretvalue123" not in red


def test_redacts_email_and_home_path():
    red, _ = S.redact(r"mail map12@gmail.com path C:\Users\map12\proj")
    assert "map12@gmail.com" not in red and r"C:\Users\map12" not in red


def test_residual_entropy_lowers_confidence():
    # un token largo no clasificado deja confidence 0 (no mandar a API).
    _, conf = S.redact("token " + "a1B2" * 10)   # 40 chars base64-ish
    assert conf == 0.0


def test_clean_text_high_confidence():
    red, conf = S.redact("arregla el bug en el parser")
    assert conf == 1.0 and red == "arregla el bug en el parser"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_redact.py -q`
Expected: FAIL with `AttributeError: module 'mmorch.sessions' has no attribute 'redact'`

- [ ] **Step 3: Write minimal implementation**

Add `import re` to the top of `mmorch/sessions.py`, then append:

```python
_RED = "[REDACTED]"
_PATTERNS = [
    re.compile(r"\b(sk|pk|gh[pousr]|xox[baprs])-[A-Za-z0-9_\-]{8,}\b"),   # api keys
    re.compile(r"(?i)\b(api[_-]?key|token|secret|password|authorization|bearer)\b\s*[:=]\s*\S+"),
    re.compile(r"\b[\w.+-]+@[\w-]+\.[\w.-]+\b"),                          # emails
    re.compile(r"(?i)C:\\Users\\[^\\\s\"']+"),                            # win home
    re.compile(r"/(?:home|Users)/[^/\s\"']+"),                           # unix home
]
# residual: token largo base64/hex no clasificado tras redactar.
_RESIDUAL = re.compile(r"\b[A-Za-z0-9+/=]{32,}\b")


def redact(text: str) -> tuple[str, float]:
    """Saca secrets antes de cualquier salida externa. confidence=0 si queda un token
    de alta entropia sin clasificar (no mandar a API). Redacta-ante-duda."""
    out = text
    for pat in _PATTERNS:
        out = pat.sub(_RED, out)
    confidence = 0.0 if _RESIDUAL.search(out.replace(_RED, "")) else 1.0
    return out, confidence
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_redact.py -q`
Expected: PASS (5 passed)

- [ ] **Step 5: Commit**

```bash
git add mmorch/sessions.py tests/test_sessions_redact.py
git commit -m "feat(sessions): secret redaction gate for external enrichment"
```

---

### Task 4: Observed difficulty → observed Cynefin domain

**Files:**
- Modify: `mmorch/sessions.py`
- Test: `tests/test_sessions_difficulty.py`

**Interfaces:**
- Consumes: `Segment` from Task 1.
- Produces: `observed_domain(seg: Segment) -> str` returning one of `"clear" | "complicated" | "complex" | "chaotic"` (same labels as `classify.CYNEFIN_CLASSES`). Deterministic from counts: tool-call volume + presence of reverts/errors. The LLM-refined version is a deferred slice.

Mapping: 0-1 tool calls and no error/revert → `clear`; 2-5 → `complicated`; >5 tool calls → `complex`; any revert or an unresolved error tool_result → `chaotic`.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_difficulty.py
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")
Seg = S.Segment


def test_one_tool_call_is_clear():
    assert S.observed_domain(Seg(request="x", tool_calls=[{"name": "Read", "input": {}}])) == "clear"


def test_few_tool_calls_complicated():
    tc = [{"name": "Edit", "input": {}} for _ in range(3)]
    assert S.observed_domain(Seg(request="x", tool_calls=tc)) == "complicated"


def test_many_tool_calls_complex():
    tc = [{"name": "Edit", "input": {}} for _ in range(8)]
    assert S.observed_domain(Seg(request="x", tool_calls=tc)) == "complex"


def test_revert_is_chaotic():
    seg = Seg(request="x", tool_calls=[{"name": "Bash", "input": {"command": "git revert HEAD"}}])
    assert S.observed_domain(seg) == "chaotic"


def test_error_result_is_chaotic():
    seg = Seg(request="x", tool_calls=[{"name": "Bash", "input": {}}],
              tool_results=[{"content": "boom", "is_error": True}])
    assert S.observed_domain(seg) == "chaotic"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_difficulty.py -q`
Expected: FAIL with `AttributeError: module 'mmorch.sessions' has no attribute 'observed_domain'`

- [ ] **Step 3: Write minimal implementation**

Append to `mmorch/sessions.py`:

```python
def observed_domain(seg: "Segment") -> str:
    """Dificultad REAL observada -> dominio Cynefin (ground-truth para calibrar)."""
    has_error = any(r.get("is_error") for r in seg.tool_results)
    has_revert = any(
        any(k in str(tc.get("input", {}).get("command", "")).lower() for k in _REVERT_TOOLS)
        for tc in seg.tool_calls)
    if has_error or has_revert:
        return "chaotic"
    n = len(seg.tool_calls)
    if n <= 1:
        return "clear"
    if n <= 5:
        return "complicated"
    return "complex"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_difficulty.py -q`
Expected: PASS (5 passed)

- [ ] **Step 5: Commit**

```bash
git add mmorch/sessions.py tests/test_sessions_difficulty.py
git commit -m "feat(sessions): observed difficulty -> Cynefin domain"
```

---

### Task 5: ingest_session — orchestrate + calibrate + idempotency

**Files:**
- Modify: `mmorch/sessions.py`
- Test: `tests/test_sessions_ingest.py`

**Interfaces:**
- Consumes: `parse_session`, `outcome_of`, `observed_domain` (Tasks 1,2,4); `cynefin_classify` from `mmorch.classify`; `record_outcome` from `mmorch.feedback`.
- Produces: `@dataclass IngestReport(session: str, segments: int, recorded: int, skipped_no_signal: int, already_ingested: bool)`. `ingest_session(path, *, router_model=DEFAULT_ROUTER, recorder=record_outcome, classifier=cynefin_classify, ledger=_LEDGER) -> IngestReport`. `recorder`, `classifier`, `ledger` are injectable for testing. The arm passed to `recorder` is `f"cynefin:{predicted_domain}"`; reward is `1.0` if predicted == observed else `0.0`; `source="claude_session"`; `context=outcome.source`.

Idempotency: the session's content hash is appended to a ledger file after a successful ingest; re-ingesting a hash already in the ledger returns `already_ingested=True` and records nothing.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_ingest.py
import sys, pathlib, json, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def _sess(tmp_path, events):
    p = tmp_path / "s.jsonl"
    p.write_text("\n".join(json.dumps(e) for e in events), encoding="utf-8")
    return p


def _calib_session(tmp_path):
    # 1 segmento: 1 tool-call (observed=clear) + tests passed (outcome positivo).
    return _sess(tmp_path, [
        {"type": "user", "message": {"role": "user", "content": "lee el archivo"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "name": "Read", "input": {}}]}},
        {"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "content": "3 passed", "is_error": False}]}},
    ])


def test_records_calibration_with_predicted_domain(tmp_path):
    rec = []
    led = tmp_path / "ledger.txt"
    # classifier predice "complex"; observed sera "clear" -> reward 0 (mismatch).
    rep = S.ingest_session(_calib_session(tmp_path),
                           recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "complex"})(),
                           ledger=led)
    assert rep.recorded == 1 and rep.segments == 1
    k = rec[0]
    assert k["arm"] == "cynefin:complex" and k["reward"] == 0.0
    assert k["source"] == "claude_session"


def test_matching_domain_rewards_one(tmp_path):
    rec = []
    rep = S.ingest_session(_calib_session(tmp_path),
                           recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "clear"})(),
                           ledger=tmp_path / "l.txt")
    assert rec[0]["reward"] == 1.0


def test_no_signal_segment_is_skipped(tmp_path):
    rec = []
    p = _sess(tmp_path, [
        {"type": "user", "message": {"role": "user", "content": "contame algo"}},
        {"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "una historia"}]}}])
    rep = S.ingest_session(p, recorder=lambda **k: rec.append(k),
                           classifier=lambda req, **k: type("R", (), {"domain": "clear"})(),
                           ledger=tmp_path / "l.txt")
    assert rep.recorded == 0 and rep.skipped_no_signal == 1 and rec == []


def test_idempotent_second_ingest_skips(tmp_path):
    rec = []
    led = tmp_path / "l.txt"
    args = dict(recorder=lambda **k: rec.append(k),
                classifier=lambda req, **k: type("R", (), {"domain": "clear"})(), ledger=led)
    p = _calib_session(tmp_path)
    S.ingest_session(p, **args)
    rep2 = S.ingest_session(p, **args)
    assert rep2.already_ingested and rep2.recorded == 0 and len(rec) == 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_ingest.py -q`
Expected: FAIL with `AttributeError: module 'mmorch.sessions' has no attribute 'ingest_session'`

- [ ] **Step 3: Write minimal implementation**

Add `import hashlib` to the top of `mmorch/sessions.py`. Append:

```python
from .classify import cynefin_classify
from .config import DEFAULT_ROUTER
from .feedback import record_outcome

_LEDGER = Path(__file__).resolve().parent.parent / "logs" / "ingested_sessions.txt"


@dataclass
class IngestReport:
    session: str
    segments: int
    recorded: int
    skipped_no_signal: int
    already_ingested: bool = False


def _resolve_latest() -> Path:
    proj = Path.home() / ".claude" / "projects"
    files = sorted(proj.rglob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not files:
        raise FileNotFoundError("no session JSONL found under ~/.claude/projects")
    return files[0]


def ingest_session(path, *, router_model: str = DEFAULT_ROUTER,
                   recorder=record_outcome, classifier=cynefin_classify,
                   ledger: Path = _LEDGER) -> IngestReport:
    """Calibra cynefin_classify contra la dificultad observada en una sesion real.
    Idempotente por hash. recorder/classifier/ledger son inyectables (tests)."""
    p = _resolve_latest() if path == "latest" else Path(path)
    raw = p.read_text(encoding="utf-8")
    h = hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]
    seen = set()
    if ledger.exists():
        seen = {ln.strip() for ln in ledger.read_text(encoding="utf-8").splitlines() if ln.strip()}
    if h in seen:
        return IngestReport(session=p.name, segments=0, recorded=0,
                            skipped_no_signal=0, already_ingested=True)

    segs = parse_session(p)
    recorded = skipped = 0
    for i, seg in enumerate(segs):
        next_req = segs[i + 1].request if i + 1 < len(segs) else ""
        out = outcome_of(seg, next_request=next_req)
        if out is None:
            skipped += 1
            continue
        predicted = classifier(seg.request, router_model=router_model).domain or "unknown"
        obs = observed_domain(seg)
        recorder(arm=f"cynefin:{predicted}", reward=1.0 if predicted == obs else 0.0,
                 source="claude_session", context=f"{out.source}:{obs}")
        recorded += 1

    ledger.parent.mkdir(parents=True, exist_ok=True)
    with open(ledger, "a", encoding="utf-8") as fh:
        fh.write(h + "\n")
    return IngestReport(session=p.name, segments=len(segs), recorded=recorded,
                        skipped_no_signal=skipped)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_ingest.py -q`
Expected: PASS (4 passed)

- [ ] **Step 5: Commit**

```bash
git add mmorch/sessions.py tests/test_sessions_ingest.py
git commit -m "feat(sessions): ingest_session calibrates router from real outcomes"
```

---

### Task 6: Public exports + MCP tool

**Files:**
- Modify: `mmorch/__init__.py`
- Modify: `mcp_server.py`
- Test: `tests/test_sessions_wiring.py`

**Interfaces:**
- Consumes: `ingest_session`, `IngestReport`, `Segment` from Task 5.
- Produces: package exports `ingest_session`, `parse_session`, `IngestReport`, `Segment`; MCP tool `mmorch_ingest_session(path: str = "latest") -> str` returning the report as JSON.

- [ ] **Step 1: Write the failing test**

```python
# tests/test_sessions_wiring.py
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))


def test_package_exports():
    m = importlib.import_module("mmorch")
    for name in ("ingest_session", "parse_session", "IngestReport", "Segment"):
        assert hasattr(m, name), name


def test_mcp_tool_exists():
    srv = importlib.import_module("mcp_server")
    assert hasattr(srv, "mmorch_ingest_session")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_wiring.py -q`
Expected: FAIL (`AssertionError: ingest_session` or no `mmorch_ingest_session`)

- [ ] **Step 3: Write minimal implementation**

In `mmorch/__init__.py`, after the spec import line (`from .spec import ...`), add:

```python
from .sessions import (ingest_session, parse_session,        # aprende de sesiones Claude
                       IngestReport, Segment)
```

And in the `__all__` list, after the `"build_spec", "spec_interview", "SpecResult",` line, add:

```python
    "ingest_session", "parse_session", "IngestReport", "Segment",
```

In `mcp_server.py`, after the spec import line (`from mmorch.spec import ...`), add:

```python
from mmorch.sessions import ingest_session as _ingest_session
```

And after the `mmorch_build_spec` tool definition, add:

```python
@mcp.tool()
def mmorch_ingest_session(path: str = "latest") -> str:
    """Learn from a Claude Code session transcript: parse it, derive deterministic
    outcomes (external signal only), and calibrate the Cynefin router against the
    observed difficulty. path="latest" picks the most recent session under
    ~/.claude/projects. 100% local, no external API, zero leak. Returns JSON
    {session, segments, recorded, skipped_no_signal, already_ingested}."""
    r = _ingest_session(path)
    return json.dumps({"session": r.session, "segments": r.segments,
                       "recorded": r.recorded, "skipped_no_signal": r.skipped_no_signal,
                       "already_ingested": r.already_ingested}, ensure_ascii=False)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_wiring.py -q`
Expected: PASS (2 passed)

- [ ] **Step 5: Run the full sessions test set + import check**

Run: `./.venv/Scripts/python.exe -m pytest tests/test_sessions_parse.py tests/test_sessions_outcome.py tests/test_sessions_redact.py tests/test_sessions_difficulty.py tests/test_sessions_ingest.py tests/test_sessions_wiring.py -q`
Then: `./.venv/Scripts/python.exe -c "import mmorch, mcp_server; print('import OK')"`
Expected: all pass + `import OK`

- [ ] **Step 6: Commit**

```bash
git add mmorch/__init__.py mcp_server.py tests/test_sessions_wiring.py
git commit -m "feat(sessions): export ingest_session + mmorch_ingest_session MCP tool"
```

---

## Deferred slices (NOT in v0)

- **Trajectories → skills:** successful tool-call sequences → `trajectory.distill_skill`.
- **Reasoning store:** redacted reasoning summary (uses Task 3 `redact` + cross-family external enrichment) → `memory` semantic notes.
- **Auto-ingest hook:** a `SessionEnd` hook that calls `mmorch_ingest_session` on the just-closed session, once redaction + label quality are validated on real data.

## Self-Review

- **Spec coverage:** parse (T1), outcome label + anti-sycophancy (T2), redaction gate (T3), observed difficulty/Cynefin (T4), router-calibration sink + idempotency (T5), MCP/exports (T6). Manual trigger = the MCP tool (T6). Deferred slices listed. All spec sections covered.
- **Placeholder scan:** no TBD/TODO; every code step is complete.
- **Type consistency:** `Segment`, `Outcome`, `IngestReport` fields and `outcome_of`/`observed_domain`/`ingest_session` signatures match across tasks; `cynefin_classify(...).domain` matches `CynefinResult.domain` in `classify.py`; `record_outcome(arm, reward, *, source, context, ...)` matches `feedback.py`.
