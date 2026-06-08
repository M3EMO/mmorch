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


def run_sandboxed(code: str, *, timeout: float = 5.0, input_text: str = "",
                  extra_files: dict[str, str] | None = None,
                  argv: list[str] | None = None) -> SandboxResult:
    """Escribe `code` a un tmp y lo corre con el python del venv, aislado. extra_files:
    {nombre: contenido} co-ubicados (ej un test file). argv: comando alternativo
    (ej ['-m','pytest','-q']); por default corre el script."""
    with tempfile.TemporaryDirectory(prefix="mmorch_sbx_") as td:
        tdp = pathlib.Path(td)
        (tdp / "_run.py").write_text(code, encoding="utf-8")
        for name, content in (extra_files or {}).items():
            (tdp / name).write_text(content, encoding="utf-8")
        # env minimo: PATH + SYSTEMROOT (Windows lo necesita pa arrancar python), nada mas.
        env = {"PATH": os.environ.get("PATH", "")}
        for k in ("SYSTEMROOT", "TEMP", "TMP"):
            if os.environ.get(k):
                env[k] = os.environ[k]
        cmd = [sys.executable] + (argv if argv is not None else ["_run.py"])
        try:
            p = subprocess.run(cmd, cwd=td, input=input_text, capture_output=True,
                               text=True, timeout=timeout, env=env)
            return SandboxResult(p.returncode == 0, p.stdout, p.stderr, p.returncode, False)
        except subprocess.TimeoutExpired as e:
            return SandboxResult(False, (e.stdout or "") if isinstance(e.stdout, str) else "",
                                 "TIMEOUT", None, True)
        except Exception as e:  # pragma: no cover
            return SandboxResult(False, "", f"sandbox error: {e}", None, False)
