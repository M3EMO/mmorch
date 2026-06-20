"""pty_session — interactive PTY sessions for the Lotus terminal.

Transport (matches the server's existing model, no hand-rolled WebSocket):
  output = SSE  (GET /pty/{sid}/stream)   ·   input = POST /pty/{sid}/input

SECURITY: this is a real interactive shell on the host. It is gated exactly like
the rest of the server — token-auth + private tailnet + you-are-the-operator. Each
session is bound to a cwd, idle-reaped after _IDLE_TIMEOUT, and killed on close.
Run ONLY behind the private tunnel with a token set (same rule as server.py).

ponytail: ConPTY via pywinpty on Windows, stdlib pty on POSIX. One reader thread
per session fans bytes out to SSE subscribers via per-subscriber queues.
"""
from __future__ import annotations

import os
import queue
import threading
import time
import uuid

_IDLE_TIMEOUT = float(os.getenv("MMORCH_PTY_IDLE", "1800"))  # 30 min
_MAX_SESSIONS = int(os.getenv("MMORCH_PTY_MAX", "8"))
_WIN = os.name == "nt"


class PtySession:
    def __init__(self, cwd: str | None = None, rows: int = 30, cols: int = 100, shell: str | None = None):
        self.id = "pty-" + uuid.uuid4().hex[:10]
        self.cwd = cwd if (cwd and os.path.isdir(cwd)) else None
        self.alive = True
        self.last = time.time()
        self.subscribers: list[queue.Queue] = []
        self._lock = threading.Lock()
        self._spawn(rows, cols, shell)
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

    # --- platform spawn / io ------------------------------------------------ #
    def _spawn(self, rows, cols, shell):
        if _WIN:
            import winpty
            self._backend = "conpty"
            self._p = winpty.PtyProcess.spawn(shell or "powershell.exe -NoLogo",
                                              cwd=self.cwd, dimensions=(rows, cols))
        else:
            import pty as _pty
            import subprocess
            self._backend = "posix"
            self._master, slave = _pty.openpty()
            self._proc = subprocess.Popen([shell or os.getenv("SHELL", "/bin/bash")],
                                          cwd=self.cwd, stdin=slave, stdout=slave, stderr=slave,
                                          preexec_fn=os.setsid, close_fds=True)
            os.close(slave)
            self._resize_posix(rows, cols)

    def _read_once(self) -> str:
        if _WIN:
            return self._p.read(2048)                       # blocking; '' / EOFError at end
        data = os.read(self._master, 2048)
        return data.decode("utf-8", "replace")

    def _is_alive(self) -> bool:
        return self._p.isalive() if _WIN else (self._proc.poll() is None)

    def _resize_posix(self, rows, cols):
        import fcntl, struct, termios
        fcntl.ioctl(self._master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    # --- lifecycle ---------------------------------------------------------- #
    def _read_loop(self):
        try:
            while self.alive and self._is_alive():
                data = self._read_once()
                if not data:
                    break
                self._broadcast(data)
        except (EOFError, OSError):
            pass
        except Exception:
            pass
        self.alive = False
        self._broadcast("\r\n\x1b[2m[process exited]\x1b[0m\r\n")

    def _broadcast(self, data: str):
        with self._lock:
            subs = list(self.subscribers)
        for q in subs:
            try:
                q.put_nowait(data)
            except queue.Full:
                pass

    def subscribe(self) -> queue.Queue:
        q: queue.Queue = queue.Queue(maxsize=2000)
        with self._lock:
            self.subscribers.append(q)
        return q

    def unsubscribe(self, q: queue.Queue):
        with self._lock:
            if q in self.subscribers:
                self.subscribers.remove(q)

    def write(self, data: str):
        self.last = time.time()
        if _WIN:
            self._p.write(data)
        else:
            os.write(self._master, data.encode("utf-8"))

    def resize(self, rows: int, cols: int):
        try:
            if _WIN:
                self._p.setwinsize(rows, cols)
            else:
                self._resize_posix(rows, cols)
        except Exception:
            pass

    def close(self):
        self.alive = False
        try:
            if _WIN:
                self._p.close(force=True)
            else:
                self._proc.terminate()
        except Exception:
            try:
                self._p.kill() if _WIN else self._proc.kill()
            except Exception:
                pass


# --- registry + idle reaper ------------------------------------------------- #
_SESSIONS: dict[str, PtySession] = {}
_SLOCK = threading.Lock()


def _reap():
    now = time.time()
    with _SLOCK:
        dead = [sid for sid, s in _SESSIONS.items()
                if not s.alive or (now - s.last) > _IDLE_TIMEOUT]
        for sid in dead:
            s = _SESSIONS.pop(sid, None)
            if s:
                s.close()


def open_session(cwd=None, rows=30, cols=100) -> PtySession:
    _reap()
    with _SLOCK:
        if len(_SESSIONS) >= _MAX_SESSIONS:
            raise RuntimeError(f"too many PTY sessions (max {_MAX_SESSIONS})")
    s = PtySession(cwd, rows, cols)
    with _SLOCK:
        _SESSIONS[s.id] = s
    return s


def get(sid: str) -> PtySession | None:
    with _SLOCK:
        return _SESSIONS.get(sid)


def close_session(sid: str) -> bool:
    with _SLOCK:
        s = _SESSIONS.pop(sid, None)
    if s:
        s.close()
        return True
    return False
