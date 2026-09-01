"""W5.2 — contrato de SCHEMA del MCP server (analogo de test_server_smoke para tools).

FastMCP genera el JSON schema de cada tool desde la firma de la funcion; si el
guard/telemetria rompiera esa firma, o un refactor dejara un param sin annotation,
el cliente MCP veria un schema que no matchea lo que la funcion acepta. Aca se
congela: params declarados == firma real, required == sin default. El error-shape
en fallo ya lo cubre test_mcp_contract (W5.1) — no se repite.
"""
from __future__ import annotations

import inspect
import os

import pytest

# El contrato de schema es del CATALOGO completo, no del perfil que se exponga:
# desde la poda 2026-08-30 el default es "core" (15 tools) y sin esto el test
# congelaria 15 en vez de 47, dejando de ver un drift en las 32 restantes.
os.environ["MMORCH_MCP_PROFILE"] = "full"

from mmorch.mcp_server import mcp

TOOLS = sorted(mcp._tool_manager.list_tools(), key=lambda t: t.name)
assert TOOLS, "el server no registro ninguna tool"

# La tabla congelada de nombres: una tool dropeada/renombrada en un refactor
# rompe ESTE assert con el diff exacto (actualizarla aca = cambio explicito).
EXPECTED_TOOL_COUNT = 47


def test_cantidad_de_tools_congelada():
    assert len(TOOLS) == EXPECTED_TOOL_COUNT, sorted(t.name for t in TOOLS)


@pytest.mark.parametrize("tool", TOOLS, ids=lambda t: t.name)
def test_schema_matchea_la_firma(tool):
    sig = inspect.signature(tool.fn)
    params = {n: p for n, p in sig.parameters.items() if n != tool.context_kwarg}
    props = set(tool.parameters.get("properties", {}))
    assert props == set(params), (
        f"{tool.name}: schema declara {sorted(props)} pero la firma es {sorted(params)}")
    requeridos = {n for n, p in params.items() if p.default is inspect.Parameter.empty}
    assert set(tool.parameters.get("required", [])) == requeridos, tool.name


@pytest.mark.parametrize("tool", TOOLS, ids=lambda t: t.name)
def test_params_anotados_y_descripcion(tool):
    # sin annotation FastMCP degrada el schema del param; sin docstring el cliente
    # MCP muestra una tool muda — ambos son drift de contrato, no estetica
    sig = inspect.signature(tool.fn)
    sin_tipo = [n for n, p in sig.parameters.items()
                if n != tool.context_kwarg and p.annotation is inspect.Parameter.empty]
    assert not sin_tipo, f"{tool.name}: params sin annotation {sin_tipo}"
    assert (tool.description or "").strip(), f"{tool.name} sin docstring"
    assert tool.fn.__name__ == tool.name, "el wrapper perdio el nombre de la tool"
