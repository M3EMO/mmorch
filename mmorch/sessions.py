"""sessions — aprende de transcripts de Claude Code. Parsea el JSONL de sesion en
segmentos de tarea, deriva un outcome determinista (label externo), estima la
dificultad observada y calibra cynefin_classify via feedback.record_outcome.
v0: 100% local, sin API externa, sin fuga. Library-only."""
from __future__ import annotations

import hashlib
import json
import re
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
        try:                                  # una linea corrupta no mata el parse
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
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
                    cur.reasoning = (cur.reasoning + " " + (b.get("text") or "")).strip()
                elif b.get("type") == "tool_use":
                    cur.tool_calls.append({"name": b.get("name", ""), "input": b.get("input", {})})
    return segments


_ACCEPT = ("funciona", "perfecto", "gracias", "dale", "joya", "excelente", "buenisimo", "anda")
_REJECT = ("no,", "mal", "rehace", "rehacer", "no funciona", "error", "esta roto", "revert")
_REVERT_TOOLS = ("revert", "undo", "checkout --")


@dataclass
class Outcome:
    reward: float
    source: str        # "tool" | "user"
    confidence: float = 1.0


# un fallo real lleva conteo NO-CERO ("1 failed", "2 errors"); "error" suelto en un log
# exitoso no cuenta (precision fix del mmorch verify de Task 2).
_TEST_FAIL = re.compile(r"[1-9]\d*\s+(failed|error)")


def _tests_signal(seg: "Segment") -> float | None:
    for r in seg.tool_results:
        c = r["content"].lower()
        if "passed" in c or "failed" in c:        # resumen tipo pytest presente
            return 0.0 if _TEST_FAIL.search(c) else 1.0
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


_RED = "[REDACTED]"
_PATTERNS = [
    re.compile(r"-----BEGIN[A-Z ]*PRIVATE KEY-----[\s\S]*?-----END[A-Z ]*PRIVATE KEY-----"),  # PEM
    re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\b"),            # JWT
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),                                  # AWS access key id
    re.compile(r"\b(sk|pk|gh[pousr]|xox[baprs])-[A-Za-z0-9_\-]{8,}\b"),   # api keys
    re.compile(r"(?i)\b(api[_-]?key|token|secret|password|authorization|bearer)\b\s*[:=]\s*\S+"),
    re.compile(r"\b[\w.+-]+@[\w-]+\.[\w.-]+\b"),                          # emails
    re.compile(r"(?i)C:\\Users\\[^\\\s\"']+"),                            # win home
    re.compile(r"/(?:home|Users)/[^/\s\"']+"),                           # unix home
]
# residual: token largo de alta entropia no clasificado tras redactar. Incluye - y _
# (UUIDs, session tokens). ponytail: secret CORTO sin marcador (ej 'hunter2' en prosa)
# es indetectable por regex sin entropia Shannon — techo conocido; el gate degrada a
# solo-determinista (no manda a API) ante duda, que es el fail-safe.
_RESIDUAL = re.compile(r"[A-Za-z0-9+/=_-]{32,}")


def redact(text: str) -> tuple[str, float]:
    """Saca secrets antes de cualquier salida externa. confidence=0 si queda un token
    de alta entropia sin clasificar (no mandar a API). Redacta-ante-duda."""
    out = text
    for pat in _PATTERNS:
        out = pat.sub(_RED, out)
    confidence = 0.0 if _RESIDUAL.search(out.replace(_RED, "")) else 1.0
    return out, confidence


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
