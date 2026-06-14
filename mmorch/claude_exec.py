"""claude_exec — ejecutor que corre en el PLAN de Claude (cupo), no por API. Invoca el
`claude` CLI headless (`-p`) dentro del repo de un proyecto, con file-tools reales (Read/
Edit/Bash en ese cwd) — algo que un API crudo de DeepSeek NO puede. Streamea los pasos
(tool-uses) al bus de eventos => se ven en vivo en el dashboard (tipo Codex).

Contradice el ahorro-de-cupo a proposito: es una ELECCION por-job. Para trabajo de coding
sobre un repo real, la calidad + file-tools de Claude valen el cupo; lo barato (DeepSeek API)
no navega archivos. El orquestador sigue siendo determinista; el juez puede seguir cross-family.

SEGURIDAD: mode='plan' (default) = read-only (--permission-mode plan), NO escribe. mode='edit'
= acceptEdits, escribe — el caller DEBE correrlo sobre una branch/worktree aislada (reversible
+ revisable). Nunca escribir directo sobre main remoto sin gate humano.
"""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

from .events import emit


def claude_bin() -> list[str]:
    """argv-prefix pa invocar el CLI. Env MMORCH_CLAUDE_BIN manda; si no, npm global .cmd."""
    env = os.getenv("MMORCH_CLAUDE_BIN")
    if env:
        return [env]
    cand = Path(os.path.expanduser("~")) / "AppData" / "Roaming" / "npm" / "claude.cmd"
    if cand.exists():
        return ["cmd", "/c", str(cand)]
    return ["claude"]   # fallback: confiar en PATH


_PERM = {"plan": "plan", "edit": "acceptEdits", "read": "plan"}


def run_claude(prompt: str, cwd: str, *, mode: str = "plan", timeout: float = 600.0,
               job_id: str = "", model: str | None = None) -> dict:
    """Corre claude -p headless en `cwd` sobre el PLAN. Streamea tool-uses al bus.
    Devuelve {ok, result, returncode, steps}. mode 'plan'=read-only, 'edit'=acceptEdits."""
    if not os.path.isdir(cwd):
        return {"ok": False, "result": f"cwd inexistente: {cwd}", "returncode": None, "steps": 0}
    pm = _PERM.get(mode, "plan")
    argv = claude_bin() + ["-p", "--output-format", "stream-json", "--verbose",
                           "--permission-mode", pm]
    if model:
        argv += ["--model", model]
    emit("job", "running", job_id=job_id, node=f"claude:{mode}", detail=f"cwd={os.path.basename(cwd)}")
    steps, result = 0, ""
    try:
        p = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
        p.stdin.write(prompt); p.stdin.close()
        for line in p.stdout:
            line = line.strip()
            if not line:
                continue
            steps += _emit_stream_line(line, job_id)
            r = _maybe_result(line)
            if r:
                result = r
        rc = p.wait(timeout=timeout)
        ok = rc == 0
    except subprocess.TimeoutExpired:
        p.kill(); emit("job", "error", job_id=job_id, node="claude", detail="TIMEOUT")
        return {"ok": False, "result": "TIMEOUT", "returncode": None, "steps": steps}
    except Exception as e:
        emit("job", "error", job_id=job_id, node="claude", detail=str(e)[:160])
        return {"ok": False, "result": str(e)[:200], "returncode": None, "steps": steps}
    emit("job", "done" if ok else "error", job_id=job_id, node="claude",
         detail=f"{steps} pasos, rc={rc}")
    return {"ok": ok, "result": result, "returncode": rc, "steps": steps}


def _emit_stream_line(line: str, job_id: str) -> int:
    """Parsea una linea de stream-json y emite un evento por tool-use/mensaje. Best-effort."""
    try:
        ev = json.loads(line)
    except Exception:
        return 0
    t = ev.get("type")
    if t == "assistant":
        for block in (ev.get("message", {}).get("content") or []):
            if isinstance(block, dict) and block.get("type") == "tool_use":
                name = block.get("name", "tool")
                inp = block.get("input", {})
                hint = inp.get("file_path") or inp.get("command") or inp.get("pattern") or ""
                emit("step", "running", job_id=job_id, node=f"claude:{name}",
                     detail=str(hint)[:120])
                return 1
    return 0


def _maybe_result(line: str) -> str:
    try:
        ev = json.loads(line)
    except Exception:
        return ""
    if ev.get("type") == "result":
        return str(ev.get("result", ""))[:4000]
    return ""
