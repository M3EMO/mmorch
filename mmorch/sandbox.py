"""sandbox — corre codigo NO confiable aislado (la compuerta del pipeline 'git-like'
sandbox -> promote). El codigo generado por un modelo se ejecuta en un subproceso
efimero; si pasa, el CALLER lo promueve a produccion (su commit). El checker es el
gate, igual que tests verdes gatean un merge.

AISLAMIENTO (honesto sobre sus limites):
  - proceso SEPARADO (no comparte memoria con mmorch).
  - cwd = directorio TEMPORAL nuevo (no toca el repo).
  - env MINIMO (sin tus secrets/API keys en el entorno del hijo).
  - timeout con KILL (mata loops infinitos).
  - stdout/stderr capturados.
  LIMITE: en Windows no hay seccomp/namespaces -> el subproceso PUEDE tocar la red y
  el filesystem fuera del cwd si se empeña. Para codigo REALMENTE hostil usar un
  container (Docker: --network none, tmpfs, ulimits, --read-only). Este runner es
  'razonablemente aislado' para codigo semi-confiable (generado por TU pipeline), NO un
  jail contra un atacante. OPT-IN, nunca en el camino por default.
"""
from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass


@dataclass
class SandboxResult:
    ok: bool                 # returncode == 0 y no timeout
    stdout: str
    stderr: str
    returncode: int | None
    timed_out: bool
    violations: tuple = ()   # patrones de policy que bloquearon (si enforce_policy)


# --- Policy allowlist/denylist (idea Hermes 'command approval') ------------- #
# Escaneo ESTATICO pre-ejecucion: si el codigo toca red/proceso/fs-peligroso, se
# rechaza ANTES de correr (no se confia en el aislamiento como unica defensa).
# Distinto de evolve.red_content_hits (eso gatea lo que se COMMITEA; esto lo que se
# EJECUTA). Default OFF (enforce_policy=False) — opt-in, como el resto del sandbox.
_POLICY_DENY = (
    r"\bimport\s+socket\b", r"\bimport\s+subprocess\b", r"\bfrom\s+subprocess\b",
    r"\bos\.system\b", r"\bos\.popen\b", r"\bos\.exec", r"\bos\.fork\b",
    r"\bimport\s+ctypes\b", r"\bimport\s+requests\b", r"\bimport\s+urllib\b",
    r"\bimport\s+http\b", r"\bsocket\.socket\b", r"\b__import__\s*\(",
    r"\bshutil\.rmtree\b", r"\bopen\s*\([^)]*['\"][wax]",   # escritura de archivos
)


def policy_violations(code: str, deny: tuple = _POLICY_DENY) -> list[str]:
    """Patrones peligrosos presentes en el codigo (red, proceso, fs-write). Vacio = limpio."""
    return [p for p in deny if re.search(p, code)]


def docker_available() -> bool:
    return shutil.which("docker") is not None


def run_sandboxed(code: str, *, timeout: float = 5.0, input_text: str = "",
                  extra_files: dict[str, str] | None = None,
                  argv: list[str] | None = None,
                  backend: str = "local", enforce_policy: bool = False,
                  deny: tuple = _POLICY_DENY) -> SandboxResult:
    """Escribe `code` a un tmp y lo corre aislado. extra_files: {nombre: contenido}
    co-ubicados (ej un test file). argv: comando alternativo (ej ['-m','pytest','-q']).

    backend:
      'local'  subproceso efimero (default; razonablemente aislado, NO jail hostil).
      'docker' container python:3.12-slim --network none --read-only (idea Hermes #12):
               aislamiento REAL (sin red, fs ro salvo tmpfs) pa codigo verdaderamente
               no-confiable. Cae a 'local' con violation si docker no esta (graceful).
    enforce_policy: escaneo estatico pre-ejecucion (idea Hermes #14); si el codigo toca
               red/proceso/fs-write -> NO se ejecuta, returncode bloqueado."""
    if enforce_policy:
        viol = policy_violations(code, deny)
        if viol:
            return SandboxResult(False, "", f"POLICY BLOCK: {viol}", None, False, tuple(viol))

    if backend == "docker":
        if not docker_available():
            return SandboxResult(False, "", "docker no disponible (instalar o backend=local)",
                                 None, False, ("docker_missing",))
        return _run_docker(code, timeout, extra_files, argv)

    with tempfile.TemporaryDirectory(prefix="mmorch_sbx_") as td:
        tdp = pathlib.Path(td)
        (tdp / "_run.py").write_text(code, encoding="utf-8")
        for name, content in (extra_files or {}).items():
            (tdp / name).write_text(content, encoding="utf-8")
        # env minimo + DETERMINISMO: PYTHONHASHSEED=0 fija el orden de hash/dict/set;
        # PYTHONDONTWRITEBYTECODE evita .pyc; sin el resto del entorno (sin secrets).
        env = {"PATH": os.environ.get("PATH", ""), "PYTHONHASHSEED": "0",
               "PYTHONDONTWRITEBYTECODE": "1", "TZ": "UTC"}
        for k in ("SYSTEMROOT", "TEMP", "TMP"):
            if os.environ.get(k):
                env[k] = os.environ[k]
        cmd = [sys.executable] + (argv if argv is not None else ["_run.py"])
        # CREATE_NO_WINDOW on Windows: no console pop/stall, and the child does NOT inherit a
        # stdio MCP server's pipes (input= gives it its own closed stdin). Lets callers spawned
        # from the MCP server (speedup) use this runner without the inherited-stdin hang.
        kw = {"creationflags": subprocess.CREATE_NO_WINDOW} if sys.platform == "win32" else {}
        try:
            p = subprocess.run(cmd, cwd=td, input=input_text, capture_output=True,
                               text=True, timeout=timeout, env=env, **kw)
            return SandboxResult(p.returncode == 0, p.stdout, p.stderr, p.returncode, False)
        except subprocess.TimeoutExpired as e:
            return SandboxResult(False, (e.stdout or "") if isinstance(e.stdout, str) else "",
                                 "TIMEOUT", None, True)
        except Exception as e:  # pragma: no cover
            return SandboxResult(False, "", f"sandbox error: {e}", None, False)


def _run_docker(code: str, timeout: float, extra_files: dict[str, str] | None,
                argv: list[str] | None) -> SandboxResult:
    """Aislamiento fuerte: --network none (sin red), --read-only + tmpfs (fs efimero),
    --memory/--pids-limit (anti fork-bomb/OOM), --cap-drop ALL. El tmp se monta solo-lectura
    salvo /tmp. Imagen liviana; el codigo corre como en local pero enjaulado por el kernel."""
    with tempfile.TemporaryDirectory(prefix="mmorch_dkr_") as td:
        tdp = pathlib.Path(td)
        (tdp / "_run.py").write_text(code, encoding="utf-8")
        for name, content in (extra_files or {}).items():
            (tdp / name).write_text(content, encoding="utf-8")
        target = argv if argv is not None else ["_run.py"]
        cmd = [
            "docker", "run", "--rm", "--network", "none", "--read-only",
            "--memory", "256m", "--pids-limit", "128", "--cap-drop", "ALL",
            "--tmpfs", "/tmp:rw,size=64m", "-v", f"{td}:/work:ro", "-w", "/work",
            "python:3.12-slim", "python", *target,
        ]
        try:
            p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout + 10)
            return SandboxResult(p.returncode == 0, p.stdout, p.stderr, p.returncode, False)
        except subprocess.TimeoutExpired:
            return SandboxResult(False, "", "TIMEOUT", None, True)
        except Exception as e:  # pragma: no cover
            return SandboxResult(False, "", f"docker error: {e}", None, False)
