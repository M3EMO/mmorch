"""server_core — shared in-process state + tiny request helpers for the server route modules.

One home for the mutable job registry and the staged-gate state, so every route group imports
the SAME objects (importing a module global shares the object by reference, not a copy). This is
what lets the routes be split into cohesive modules without circular imports: leaf route modules
depend on server_core, and server.py depends on both.

Durabilidad (W3.2): el registro de jobs vive in-memory (fuente de verdad del Kanban) pero cada
alta y cada cambio de status se espeja append-only en logs/jobs.jsonl. Un restart post-crash
replayea ese log y lista los jobs no-terminales como 'interrupted' en vez de perderlos. Es
persistencia minima del REGISTRO, no un job queue: los pasos/checkpoints ya son de workflow_store.
"""
from __future__ import annotations

import hmac
import json
import logging
import os
import threading
import time
from pathlib import Path

logger = logging.getLogger(__name__)

_JOBS_LOCK = threading.Lock()
_GATES: dict[str, dict] = {}   # graft G6: per-job staged gate state

# Estados que ya no cambian: en el replay post-crash no se recargan (el registro
# vivo es el Kanban del proceso actual; el historial completo queda en el jsonl).
_TERMINAL = {"done", "error", "approved", "escalate"}
# Subconjunto serializable del job (cancel/pause/state son objetos vivos del proceso).
_PERSIST_KEYS = ("status", "kind", "title", "ts", "host", "engine", "parent")


def _token_ok(request) -> bool:
    """Auth del server: token OBLIGATORIO y SOLO por header (Authorization: Bearer
    o X-Token). Sin MMORCH_SERVER_TOKEN configurado NO hay 'modo dev': se niega
    todo (main() ya rehusa arrancar antes; esto cubre apps armadas sin main()).
    Nunca por query string: las URLs terminan en logs, history y referrers."""
    want = os.getenv("MMORCH_SERVER_TOKEN", "")
    if not want:
        return False
    auth = request.headers.get("authorization", "")
    got = auth[7:].strip() if auth.lower().startswith("bearer ") else \
        request.headers.get("x-token", "")
    # compare_digest: comparacion en tiempo constante (sin oraculo de prefijos)
    return hmac.compare_digest(got.encode(), want.encode())


def _budget_block():
    """Return a 402 JSONResponse if a hard budget policy is exceeded, else None (graft G5)."""
    from starlette.responses import JSONResponse
    from .budget_policy import blocking_incident
    inc = blocking_incident()
    if inc:
        return JSONResponse(
            {"error": f"budget hard-stop on '{inc['scope']}' (${inc['spent']} / ${inc['limit']})",
             "incident": inc}, status_code=402)
    return None


# --- jobs in-process + espejo durable --------------------------------------- #
def _jobmeta(kind: str, title: str, **extra) -> dict:
    """Registro de job con title/ts/host -> alimenta el Kanban (columnas por status)."""
    return {"status": "running", "kind": kind, "title": (title or kind)[:80],
            "ts": time.time(), "host": os.getenv("MMORCH_SERVER_HOST", "local"), **extra}


def _jobs_log_path() -> Path:
    from .paths import logs_dir   # lazy: respeta un MMORCH_HOME seteado post-import
    return logs_dir() / "jobs.jsonl"


def _persist(jid: str, meta: dict) -> None:
    """Append del estado serializable del job a jobs.jsonl. Fail-open: la
    durabilidad jamas debe tirar el request/thread que muta el job."""
    try:
        rec: dict = {"id": jid, "logged_ts": time.time()}
        for k in _PERSIST_KEYS:
            v = meta.get(k)
            if v is not None:
                rec[k] = v
        with _jobs_log_path().open("a", encoding="utf-8") as f:
            f.write(json.dumps(rec, default=str) + "\n")
    except Exception as e:
        logger.warning("jobs.jsonl: no se pudo persistir %s: %s", jid, e)


class _JobMeta(dict):
    """Job dict que espeja cada cambio de 'status' al jsonl — el unico campo cuyo
    historial importa post-crash. heartbeat/state mutan seguido y NO se persisten
    (evita amplificar escrituras sin ganar durabilidad util)."""
    job_id: str = ""

    def __setitem__(self, key, value) -> None:
        super().__setitem__(key, value)
        if key == "status" and self.job_id:
            _persist(self.job_id, self)


class _JobRegistry(dict):
    """Registro in-memory + espejo durable. Interceptar __setitem__ aca evita
    tocar los ~30 call sites que mutan _JOBS: el registro ES el seam de
    persistencia, no cada handler."""

    def __setitem__(self, jid: str, meta: dict) -> None:
        if not isinstance(meta, _JobMeta):
            meta = _JobMeta(meta)
        meta.job_id = jid
        super().__setitem__(jid, meta)
        _persist(jid, meta)


_JOBS: _JobRegistry = _JobRegistry()


def load_interrupted_jobs() -> list[str]:
    """Replay de jobs.jsonl al arrancar: el ultimo registro por job manda. Los
    no-terminales del proceso anterior reaparecen ('paused' tal cual — sigue
    resumible; el resto como 'interrupted': el proceso que los corria murio).
    Los terminales quedan solo en el historial. No pisa un job ya vivo."""
    path = _jobs_log_path()
    try:
        lines = path.read_text(encoding="utf-8").splitlines() if path.exists() else []
    except OSError as e:
        logger.warning("jobs.jsonl: no se pudo leer para el replay: %s", e)
        return []
    last: dict[str, dict] = {}
    for ln in lines:
        try:
            rec = json.loads(ln)
        except json.JSONDecodeError:
            continue   # linea cortada por un crash a mitad de write: se ignora
        if isinstance(rec, dict) and isinstance(rec.get("id"), str):
            last[rec["id"]] = rec
    loaded: list[str] = []
    with _JOBS_LOCK:
        for jid, rec in last.items():
            if jid in _JOBS or rec.get("status") in _TERMINAL:
                continue
            meta = {k: rec[k] for k in _PERSIST_KEYS if k in rec}
            if meta.get("status") != "paused":
                meta["status"] = "interrupted"
            _JOBS[jid] = meta   # persiste el nuevo estado -> replay estable
            loaded.append(jid)
    return sorted(loaded)
