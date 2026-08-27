"""W2.2 — perfil de tools del MCP server (MMORCH_MCP_PROFILE).

Cursor tiene un techo practico de ~40 tools por server; el perfil "core" debe
quedar <= 40 y "full" (default) exponer todo. El perfil se lee a import-time,
asi que cada perfil se mide en un subprocess fresco (mismo contrato real que
un server stdio lanzado por Cursor/Claude).
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

_LIST_CODE = (
    "import json\n"
    "from mmorch.mcp_server import mcp\n"
    "print(json.dumps(sorted(t.name for t in mcp._tool_manager.list_tools())))\n"
)


def _tool_names(tmp_path, profile: str | None) -> list[str]:
    env = dict(os.environ)
    # aislar estado: los modulos anclan paths a import-time via MMORCH_HOME
    env["MMORCH_HOME"] = str(tmp_path)
    env.pop("MMORCH_MCP_PROFILE", None)
    if profile is not None:
        env["MMORCH_MCP_PROFILE"] = profile
    out = subprocess.run([sys.executable, "-c", _LIST_CODE], env=env, cwd=REPO,
                         capture_output=True, text=True, check=True)
    return json.loads(out.stdout.strip().splitlines()[-1])


def test_full_expone_todas_las_tools(tmp_path):
    full = _tool_names(tmp_path, "full")
    # 46 al escribir W2.2; >= evita romper el test al agregar una tool nueva
    assert len(full) >= 46
    assert "mmorch_ingest_session" in full  # las excluidas de core siguen en full


def test_default_es_full(tmp_path):
    assert _tool_names(tmp_path, None) == _tool_names(tmp_path, "full")


def test_core_cabe_en_cursor_y_es_subconjunto(tmp_path):
    core = set(_tool_names(tmp_path, "core"))
    full = set(_tool_names(tmp_path, "full"))
    assert len(core) <= 40, f"core excede el techo de Cursor: {len(core)}"
    assert core < full  # subconjunto estricto: mismo codigo, menos registro


def test_core_curado_mantiene_lo_esencial_y_corta_lo_nicho(tmp_path):
    core = set(_tool_names(tmp_path, "core"))
    # el flujo diario documentado en cursor-setup.md tiene que estar
    esenciales = {"mmorch_fan_out", "mmorch_adversarial_verify", "mmorch_route",
                  "mmorch_cascade", "mmorch_recall", "mmorch_remember",
                  "mmorch_build_spec", "mmorch_budget_status", "mmorch_error_rates"}
    assert esenciales <= core
    # Claude-Code-only y experimentales fuera (matriz canonica: ingest es ciego en Cursor)
    fuera = {"mmorch_ingest_session", "mmorch_session_playbooks",
             "mmorch_evolve_nightly", "mmorch_evolve_self", "mmorch_innovate",
             "mmorch_autoresearch"}
    assert not (fuera & core)
