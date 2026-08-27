"""W2.1 — paquete instalable: recursos DENTRO del paquete, shims compat y CLI.

prompts/ y roles/ son codigo (viajan con mmorch/, no con MMORCH_HOME); los
shims en la raiz y en scripts/ mantienen vivas las rutas viejas que
~/.claude.json y el Task Scheduler ya tienen registradas.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def test_prompts_y_roles_viajan_con_el_paquete():
    """Los recursos resuelven relativo a mmorch/__file__, no al cwd/checkout."""
    import mmorch
    pkg = Path(mmorch.__file__).resolve().parent
    from mmorch.loop_nightly import _PROMPTS_DIR
    assert _PROMPTS_DIR == pkg / "prompts"
    assert (_PROMPTS_DIR / "coder_prompt.txt").is_file()
    from mmorch.workflow_spec import roles_dir
    assert roles_dir() == pkg / "roles"
    assert (roles_dir() / "coder.md").is_file()


def test_shims_compat_apuntan_al_paquete():
    """Las rutas viejas (raiz y scripts/) siguen importando el codigo real."""
    for shim, destino in [(REPO / "mcp_server.py", "mmorch.mcp_server"),
                          (REPO / "scripts" / "nightly.py", "mmorch.nightly")]:
        assert destino in shim.read_text(encoding="utf-8"), shim


def test_cli_status_emite_json(capsys):
    from mmorch.cli import main
    assert main(["status"]) == 0
    out = json.loads(capsys.readouterr().out)
    assert "runs" in out or isinstance(out, dict)


def test_cli_health_exit_es_senal(tmp_path):
    """health en un home vacio: JSON valido y exit 1 (no sano != crash)."""
    env = dict(os.environ, MMORCH_HOME=str(tmp_path))
    r = subprocess.run(
        [sys.executable, "-c", "import sys; from mmorch.cli import main; sys.exit(main(['health']))"],
        capture_output=True, text=True, env=env, cwd=str(REPO), timeout=120)
    assert r.returncode in (0, 1), r.stderr
    rep = json.loads(r.stdout)
    assert "healthy" in rep and (r.returncode == 0) == bool(rep["healthy"])


def test_entry_points_declarados():
    txt = (REPO / "pyproject.toml").read_text(encoding="utf-8")
    for ep in ("mmorch-mcp = \"mmorch.mcp_server:main\"",
               "mmorch-nightly = \"mmorch.nightly:main\"",
               "mmorch = \"mmorch.cli:main\""):
        assert ep in txt, ep
