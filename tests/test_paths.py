"""W1.1 — MMORCH_HOME: el estado resuelve via mmorch.paths, no via el checkout.

Dos gates:
1. Aislamiento real: con MMORCH_HOME apuntando a un tmpdir, el estado se escribe
   ahi (subprocess, porque los modulos anclan sus paths a import-time).
2. Anti-regresion: ningun modulo fuera de paths.py ancla estado con
   `Path(__file__).parents[1]` / `parent.parent`; los usos de CODIGO legitimos
   viven en una allowlist explicita.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# Modulos con anclas al repo LEGITIMAS (operan sobre el codigo del checkout,
# no sobre estado): prompts/roles del paquete, introspeccion del repo,
# self-evolution que edita el propio codigo, y el gate humano GOAL.md.
_ALLOWLIST = {
    "paths.py",          # el unico autorizado a resolver la raiz
    "docgen.py",         # lee README/mcp_server/tests del repo (codigo)
    "evolve.py",         # self-evolution: edita el codigo del checkout
    "evolve_findings.py",  # default root para escanear el repo (codigo)
    "goal.py",           # GOAL.md/GOAL.hash viven versionados en el repo
    "loop_nightly.py",   # prompts/ del paquete (codigo)
    "workflow_spec.py",  # roles/ y workflows/ del paquete (codigo)
}

# laxo a proposito: cualquier variante de anclar la raiz al archivo cuenta
_ANCHOR = re.compile(r"__file__.*(parents\[1\]|parent\.parent)")


def test_estado_va_a_mmorch_home(tmp_path):
    """Con MMORCH_HOME seteado, escribir estado cae en el tmpdir y no en el repo."""
    home = tmp_path / "hogar"
    code = (
        "import mmorch.paths as p, mmorch.metrics as m, mmorch.feedback as f\n"
        "print(p.home())\n"
        "m.log_event(pattern='t', node='t', model='t', family='t',\n"
        "            in_tokens=1, out_tokens=1, cost_usd=0.0, latency_s=0.0)\n"
        "print(m.log_path())\n"
        "print(f._FEEDBACK_LOG)\n"
    )
    env = dict(os.environ, MMORCH_HOME=str(home))
    r = subprocess.run([sys.executable, "-c", code], capture_output=True,
                       text=True, env=env, cwd=str(REPO), timeout=120)
    assert r.returncode == 0, r.stderr
    lines = r.stdout.strip().splitlines()
    resolved = home.resolve()
    assert Path(lines[0]) == resolved
    # el log de metrics existe DENTRO del home, no en el repo
    log = Path(lines[1])
    assert log.is_relative_to(resolved) and log.exists()
    assert Path(lines[2]).is_relative_to(resolved)


def test_sin_env_default_es_el_checkout():
    """Sin MMORCH_HOME, home() = raiz del checkout (no romper el layout actual)."""
    env = {k: v for k, v in os.environ.items() if k != "MMORCH_HOME"}
    r = subprocess.run(
        [sys.executable, "-c", "import mmorch.paths as p; print(p.home())"],
        capture_output=True, text=True, env=env, cwd=str(REPO), timeout=120)
    assert r.returncode == 0, r.stderr
    assert Path(r.stdout.strip()) == REPO


def test_gate_sin_anclas_de_estado_fuera_de_paths():
    """Grep-gate: prohibido `Path(__file__).parents[1]` fuera de paths.py.

    Si un modulo NUEVO necesita la raiz del repo por ser genuinamente de
    CODIGO, se agrega a la allowlist explicita de arriba — nunca en silencio.
    """
    ofensores = []
    for py in sorted((REPO / "mmorch").glob("*.py")):
        if py.name in _ALLOWLIST:
            continue
        if _ANCHOR.search(py.read_text(encoding="utf-8")):
            ofensores.append(py.name)
    assert not ofensores, (
        f"anclas de estado fuera de paths.py: {ofensores} — usar mmorch.paths "
        "(home/data_dir/logs_dir/db_path) o justificar en la allowlist del test")
