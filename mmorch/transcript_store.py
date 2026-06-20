"""transcript_store — per-job inter-agent transcript (in-memory).

The JSONL audit is the durable record; this is the live, query-friendly view the
Lotus client reads (GET /transcript/{job_id}) and the SSE 'transcript' event mirrors.
ponytail: in-memory dict + lock — transcripts are ephemeral UI state, not durable truth.
"""
from __future__ import annotations

import threading

from .events import emit

_T: dict[str, list] = {}
_LOCK = threading.Lock()


def append(job_id: str, model: str, role: str, text: str, *, cap: int = 4000) -> dict:
    item = {"model": (model or "?"), "role": (role or "agent"), "text": (text or "")[:cap]}
    with _LOCK:
        _T.setdefault(job_id, []).append(item)
    emit("transcript", "info", job_id=job_id, node=item["model"], detail=item["role"])
    return item


def get(job_id: str) -> list:
    with _LOCK:
        return list(_T.get(job_id, []))
