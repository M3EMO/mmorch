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
