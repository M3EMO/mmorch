import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))


def test_package_exports():
    m = importlib.import_module("mmorch")
    for name in ("ingest_session", "parse_session", "IngestReport", "Segment"):
        assert hasattr(m, name), name


def test_mcp_tool_exists():
    srv = importlib.import_module("mcp_server")
    assert hasattr(srv, "mmorch_ingest_session")
