"""Mine human decisions from Claude Code transcripts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from mmorch.sessions import parse_transcript, redact_secrets


def mine_decisions(transcript_path: str) -> list[dict]:
    """Mine assistant question + short user decision pairs from a transcript."""
    decisions: list[dict] = []
    try:
        messages = parse_transcript(transcript_path)
    except Exception:
        return []

    for i, msg in enumerate(messages):
        if msg.get("role") != "assistant":
            continue
        text = _get_text(msg)
        if not text or not _is_question(text):
            continue
        if i + 1 >= len(messages):
            break
        next_msg = messages[i + 1]
        if next_msg.get("role") != "user":
            continue
        decision = _get_text(next_msg)
        if not decision or len(decision) >= 240:
            continue
        decisions.append(
            {
                "question": redact_secrets(text[-1200:]),
                "decision": redact_secrets(decision),
                "ts": msg.get("ts"),
            }
        )
    return decisions


def ingest_decisions(
    transcript_path: str, *, logs_dir: str = "logs"
) -> dict:
    """Mine decisions, dedup, and append to decision_samples.jsonl."""
    try:
        mined = mine_decisions(transcript_path)
        log_path = Path(logs_dir) / "decision_samples.jsonl"
        log_path.parent.mkdir(parents=True, exist_ok=True)

        existing_hashes: set[str] = set()
        if log_path.exists():
            for line in log_path.read_text().splitlines():
                try:
                    existing_hashes.add(json.loads(line).get("hash", ""))
                except json.JSONDecodeError:
                    continue

        new_count = 0
        with log_path.open("a") as f:
            for sample in mined:
                hash_key = hashlib.sha256(
                    f"{sample['question']}{sample['decision']}".encode()
                ).hexdigest()
                if hash_key in existing_hashes:
                    continue
                existing_hashes.add(hash_key)
                new_count += 1
                f.write(
                    json.dumps({**sample, "hash": hash_key}) + "\n"
                )
        return {"mined": len(mined), "new": new_count}
    except Exception:
        return {"mined": 0, "new": 0}


def _get_text(msg: dict[str, Any]) -> str:
    """Extract text content from a message dict."""
    content = msg.get("content", [])
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, dict) and item.get("type") == "text":
                parts.append(item.get("text", ""))
        return "".join(parts)
    return ""


def _is_question(text: str) -> bool:
    """Check if assistant text looks like a question."""
    return "?" in text or any(
        f"{n}." in text for n in range(1, 10)
    ) or "opcion" in text.lower()
