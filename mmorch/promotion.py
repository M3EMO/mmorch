"""Durable fail-closed state machine for autonomous code promotions."""

from __future__ import annotations

import json
import os
import re
import time
import uuid
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

from .iohelpers import atomic_write_json


SCHEMA_VERSION = 1
ACTIVE = {"candidate", "merged", "observing", "reverting", "reverted"}
TERMINAL = {"accepted", "halted"}
TRANSITIONS = {
    "candidate": {"merged", "halted"},
    "merged": {"observing", "reverting", "halted"},
    "observing": {"accepted", "reverting", "halted"},
    "reverting": {"reverted", "halted"},
    "reverted": {"halted"},
    "accepted": set(),
    "halted": set(),
}
_ID = re.compile(r"^[a-zA-Z0-9._-]{1,80}$")


class PromotionError(RuntimeError):
    """Base error: callers must treat every instance as fail-closed."""


class PromotionBusy(PromotionError):
    """Another process owns the promotion store lock."""


class PromotionCorrupt(PromotionError):
    """Durable state is malformed or violates the state-machine schema."""


def _event(status: str, *, now: float, evidence: dict | None = None) -> dict:
    return {
        "kind": "auto_action",
        "status": status,
        "ts": now,
        "evidence": evidence or {},
    }


def _validate(state: dict) -> dict:
    required = {
        "schema", "id", "status", "branch", "base_sha", "diff_hash",
        "created_at", "updated_at", "history",
    }
    missing = required - set(state)
    if missing:
        raise PromotionCorrupt(f"promotion state missing fields: {sorted(missing)}")
    if state["schema"] != SCHEMA_VERSION:
        raise PromotionCorrupt(f"unsupported promotion schema: {state['schema']!r}")
    if not isinstance(state["id"], str) or not _ID.fullmatch(state["id"]):
        raise PromotionCorrupt("invalid promotion id")
    if state["status"] not in TRANSITIONS:
        raise PromotionCorrupt(f"invalid promotion status: {state['status']!r}")
    if not all(isinstance(state[k], str) and state[k] for k in ("branch", "base_sha", "diff_hash")):
        raise PromotionCorrupt("branch, base_sha and diff_hash must be non-empty strings")
    if not isinstance(state["history"], list) or not state["history"]:
        raise PromotionCorrupt("promotion history must be a non-empty list")
    return state


def transition(
    state: dict,
    target: str,
    *,
    now: float | None = None,
    evidence: dict | None = None,
    fields: dict | None = None,
) -> dict:
    """Return the next state; repeating the current target is an idempotent no-op."""
    current = _validate(dict(state))
    if target == current["status"]:
        return current
    if target not in TRANSITIONS[current["status"]]:
        raise PromotionError(f"invalid transition: {current['status']} -> {target}")
    ts = time.time() if now is None else float(now)
    nxt = {
        **current,
        **(fields or {}),
        "status": target,
        "updated_at": ts,
        "history": [*current["history"], _event(target, now=ts, evidence=evidence)],
    }
    return _validate(nxt)


class PromotionStore:
    """One active promotion pointer plus immutable per-promotion identity files."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.states = self.root / "promotions"
        self.active_path = self.root / "promotion_active.json"
        self.lock_path = self.root / "promotion.lock"

    @contextmanager
    def locked(self) -> Iterator[None]:
        self.root.mkdir(parents=True, exist_ok=True)
        try:
            fd = os.open(self.lock_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        except FileExistsError as exc:
            raise PromotionBusy(
                f"promotion lock exists: {self.lock_path}; inspect before removing"
            ) from exc
        try:
            os.write(fd, f"{os.getpid()} {time.time()}\n".encode())
            os.close(fd)
            fd = -1
            yield
        finally:
            if fd >= 0:
                os.close(fd)
            try:
                self.lock_path.unlink()
            except FileNotFoundError:
                pass

    def _state_path(self, promotion_id: str) -> Path:
        if not _ID.fullmatch(promotion_id):
            raise PromotionCorrupt("invalid promotion id in active pointer")
        return self.states / f"{promotion_id}.json"

    @staticmethod
    def _load_json(path: Path, *, label: str) -> dict:
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError as exc:
            raise PromotionCorrupt(f"{label} missing: {path}") from exc
        except (OSError, json.JSONDecodeError) as exc:
            raise PromotionCorrupt(f"{label} unreadable: {path}: {exc}") from exc
        if not isinstance(value, dict):
            raise PromotionCorrupt(f"{label} must be a JSON object")
        return value

    def load(self, promotion_id: str | None = None) -> dict | None:
        if promotion_id is None:
            if not self.active_path.exists():
                return None
            pointer = self._load_json(self.active_path, label="active promotion pointer")
            promotion_id = pointer.get("id")
            if not isinstance(promotion_id, str):
                raise PromotionCorrupt("active promotion pointer has no valid id")
        return _validate(self._load_json(self._state_path(promotion_id), label="promotion state"))

    def begin(
        self,
        *,
        branch: str,
        base_sha: str,
        diff_hash: str,
        promotion_id: str | None = None,
        now: float | None = None,
        evidence: dict | None = None,
        fields: dict | None = None,
    ) -> dict:
        with self.locked():
            prior = self.load()
            if prior is not None and prior["status"] != "accepted":
                raise PromotionBusy(
                    f"promotion {prior['id']} is {prior['status']}; only accepted may be superseded"
                )
            pid = promotion_id or uuid.uuid4().hex[:16]
            if not _ID.fullmatch(pid):
                raise PromotionCorrupt("invalid promotion id")
            path = self._state_path(pid)
            if path.exists():
                existing = self.load(pid)
                if existing is None:
                    raise PromotionCorrupt(f"promotion state disappeared: {pid}")
                if (
                    existing["branch"] == branch
                    and existing["base_sha"] == base_sha
                    and existing["diff_hash"] == diff_hash
                ):
                    return existing
                raise PromotionCorrupt(f"promotion id already exists with different identity: {pid}")
            ts = time.time() if now is None else float(now)
            state = _validate({
                **(fields or {}),
                "schema": SCHEMA_VERSION,
                "id": pid,
                "status": "candidate",
                "branch": branch,
                "base_sha": base_sha,
                "diff_hash": diff_hash,
                "created_at": ts,
                "updated_at": ts,
                "history": [_event("candidate", now=ts, evidence=evidence)],
            })
            atomic_write_json(path, state, indent=2)
            atomic_write_json(self.active_path, {"schema": SCHEMA_VERSION, "id": pid}, indent=2)
            return state

    def advance(
        self,
        target: str,
        *,
        expected: str | None = None,
        now: float | None = None,
        evidence: dict | None = None,
        fields: dict | None = None,
    ) -> dict:
        with self.locked():
            state = self.load()
            if state is None:
                raise PromotionCorrupt("cannot advance without an active promotion")
            if expected is not None and state["status"] != expected:
                raise PromotionBusy(
                    f"expected promotion status {expected}, found {state['status']}"
                )
            nxt = transition(state, target, now=now, evidence=evidence, fields=fields)
            if nxt is not state:
                atomic_write_json(self._state_path(nxt["id"]), nxt, indent=2)
            return nxt

    def record(
        self,
        *,
        action: str,
        expected: str | None = None,
        now: float | None = None,
        evidence: dict | None = None,
        fields: dict | None = None,
    ) -> dict:
        """Persist evidence without changing status (observation samples, recovery probes)."""
        with self.locked():
            state = self.load()
            if state is None:
                raise PromotionCorrupt("cannot record without an active promotion")
            if expected is not None and state["status"] != expected:
                raise PromotionBusy(
                    f"expected promotion status {expected}, found {state['status']}"
                )
            ts = time.time() if now is None else float(now)
            nxt = _validate({
                **state,
                **(fields or {}),
                "updated_at": ts,
                "history": [
                    *state["history"],
                    {
                        "kind": "auto_action",
                        "status": state["status"],
                        "action": action,
                        "ts": ts,
                        "evidence": evidence or {},
                    },
                ],
            })
            atomic_write_json(self._state_path(nxt["id"]), nxt, indent=2)
            return nxt
