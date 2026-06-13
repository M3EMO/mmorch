"""events — bus de progreso in-process pa la UI live (nivel 3). El orquestador emite
eventos (call/step/job: pending->running->done) y los suscriptores (SSE del server) los
reciben en tiempo real. SIN deps. Si no hay server/suscriptores, emit() es casi no-op
(solo guarda en un ring buffer chico) — la librería batch NO se entera ni se bloquea.

Diseño: el SERVER corre los jobs IN-PROCESS (importa mmorch y llama fan_out/rubric_loop),
así tiene los eventos en memoria y los streamea — cero lectura cross-process del JSONL
(evita la race que el verificador cross-family marcó). El JSONL sigue siendo el audit durable.
"""
from __future__ import annotations

import queue
import threading
import time
from collections import deque
from dataclasses import dataclass, asdict, field


@dataclass
class Event:
    type: str                 # job | call | step
    status: str               # pending | running | done | error | gate
    job_id: str = ""
    node: str = ""            # modelo/rol/checker
    ts: float = 0.0
    detail: str = ""
    extra: dict = field(default_factory=dict)

    def to_dict(self) -> dict:
        return asdict(self)


class EventBus:
    """Pub/sub thread-safe. subscribe() -> Queue; publish() empuja a todas + ring buffer."""
    def __init__(self, ring: int = 500):
        self._subs: list[queue.Queue] = []
        self._lock = threading.Lock()
        self._ring: deque = deque(maxlen=ring)

    def subscribe(self) -> queue.Queue:
        q: queue.Queue = queue.Queue(maxsize=1000)
        with self._lock:
            self._subs.append(q)
        return q

    def unsubscribe(self, q: queue.Queue) -> None:
        with self._lock:
            if q in self._subs:
                self._subs.remove(q)

    def publish(self, ev: Event) -> None:
        if not ev.ts:
            ev.ts = time.time()
        with self._lock:
            self._ring.append(ev)
            subs = list(self._subs)
        for q in subs:
            try:
                q.put_nowait(ev)
            except queue.Full:
                pass   # suscriptor lento: dropea, no bloquea el orquestador

    def recent(self, n: int = 100) -> list[Event]:
        with self._lock:
            return list(self._ring)[-n:]

    def has_subscribers(self) -> bool:
        with self._lock:
            return bool(self._subs)


_BUS = EventBus()


def bus() -> EventBus:
    return _BUS


def emit(type: str, status: str, *, job_id: str = "", node: str = "", detail: str = "",
         **extra) -> None:
    """Helper barato pal orquestador. Siempre seguro (no rompe la librería sin server)."""
    try:
        _BUS.publish(Event(type=type, status=status, job_id=job_id, node=node,
                           detail=detail, extra=extra))
    except Exception:
        pass
