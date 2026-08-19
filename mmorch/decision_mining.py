"""Mineria de DECISIONES humanas desde transcripts de Claude Code.

El criterio del usuario ("dale", "no", "opcion 2", "va asi") es la señal GOLD
del flywheel de entrenamiento y hoy se tira. Un par = (pregunta del assistant,
respuesta corta del usuario) — o sea: fin del reasoning de un Segment +
request del Segment siguiente (parse_session de sessions.py ya segmenta asi).
Ambos lados pasan por redact() antes de persistir.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from mmorch.sessions import parse_session, redact

_MAX_DECISION_CHARS = 240
_Q_MARKERS = ("?", "1.", "opcion", "opción")


def _tail(text: str, n: int) -> str:
    """Ultimos n chars pero arrancando en un borde legible: cortar a lo bruto
    dejaba el 58% de las muestras empezando a mitad de palabra ("n progreso"),
    y un ejemplo de entrenamiento que arranca cortado enseña ruido."""
    if len(text) <= n:
        return text
    tail = text[-n:]
    for sep in ("\n\n", "\n", ". "):
        i = tail.find(sep)
        if 0 <= i < n // 3:            # solo si el borde esta cerca del inicio
            return tail[i + len(sep):].lstrip()
    i = tail.find(" ")
    return tail[i + 1:] if 0 <= i < 40 else tail


def mine_decisions(transcript_path: str) -> list[dict]:
    """Pares (question, decision) del transcript; tolerante y fail-open."""
    try:
        segments = parse_session(transcript_path)
    except OSError:
        return []
    out = []
    for prev, nxt in zip(segments, segments[1:], strict=False):
        question = (prev.reasoning or "").strip()
        decision = (nxt.request or "").strip()
        if not question or not decision:
            continue
        if len(decision) >= _MAX_DECISION_CHARS:
            continue
        low = question.lower()
        if not any(m in low for m in _Q_MARKERS):
            continue
        out.append({"question": redact(_tail(question, 1200))[0],
                    "decision": redact(decision)[0],
                    "ts": None})
    return out


def ingest_decisions(transcript_path: str, *, logs_dir: str = "logs") -> dict:
    """mine + dedup por hash + append a decision_samples.jsonl. Fail-open."""
    try:
        mined = mine_decisions(transcript_path)
        path = Path(logs_dir) / "decision_samples.jsonl"
        seen = set()
        if path.exists():
            for line in path.read_text(encoding="utf-8").splitlines():
                try:
                    seen.add(json.loads(line).get("hash"))
                except json.JSONDecodeError:
                    continue
        path.parent.mkdir(parents=True, exist_ok=True)
        new = 0
        with open(path, "a", encoding="utf-8") as f:
            for m in mined:
                h = hashlib.sha256(
                    (m["question"] + "\x00" + m["decision"]).encode()).hexdigest()
                if h in seen:
                    continue
                seen.add(h)
                f.write(json.dumps({**m, "hash": h}, ensure_ascii=False) + "\n")
                new += 1
        return {"mined": len(mined), "new": new}
    except Exception:
        return {"mined": 0, "new": 0}
