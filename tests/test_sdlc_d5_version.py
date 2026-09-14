"""D5 (reparacion de mmorch por modulos): la version del servidor MCP no depende de la metadata instalada.

En un git worktree `importlib.metadata.version("mmorch")` levanta PackageNotFoundError y el server
quedaba sin version propia. `mmorch_version()` cae al `version = "..."` de [project] en el
pyproject.toml de `mmorch.paths.repo_root()`. Cero API, cero red.
"""
import importlib.metadata
import re

import mmorch.mcp_server as M
from mmorch.paths import repo_root


def _pyproject_version() -> str:
    txt = (repo_root() / "pyproject.toml").read_text(encoding="utf-8")
    return re.search(r'(?m)^version\s*=\s*"([^"]+)"', txt).group(1)


def test_mmorch_version_cae_a_pyproject_sin_metadata(monkeypatch):
    def _boom(name):
        raise importlib.metadata.PackageNotFoundError(name)
    monkeypatch.setattr(importlib.metadata, "version", _boom)
    assert M.mmorch_version() == _pyproject_version()


def test_mmorch_version_usa_metadata_cuando_existe(monkeypatch):
    monkeypatch.setattr(importlib.metadata, "version", lambda name: "9.9.9-test")
    assert M.mmorch_version() == "9.9.9-test"


def test_server_expone_version_no_vacia():
    assert M.mcp._mcp_server.version and M.mcp._mcp_server.version == M.mmorch_version()
