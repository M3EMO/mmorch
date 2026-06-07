"""docgen: auto-README desde introspeccion. Idempotente."""
import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.docgen as DG


def test_stats_counts_real_package():
    s = DG.stats()
    assert s["modules"] >= 10 and s["tools"] >= 5 and s["tests"] >= 20


def test_module_table_includes_known_modules():
    t = DG.module_table()
    assert "mmorch/route.py" in t and "mmorch/cascade.py" in t and "mmorch/patterns.py" in t
    assert "docgen.py" not in t  # se excluye a si mismo


def test_mcp_tools_introspected():
    ts = DG.mcp_tools()
    assert "mmorch_fan_out" in ts and "mmorch_cascade" in ts


def test_update_readme_idempotent(tmp_path):
    p = tmp_path / "R.md"
    p.write_text(
        "# x\n<!-- mmorch:auto:stats -->\nVIEJO\n<!-- /mmorch:auto:stats -->\n"
        "<!-- mmorch:auto:modules -->\n<!-- /mmorch:auto:modules -->\n", encoding="utf-8")
    up1 = DG.update_readme(p)
    txt1 = p.read_text(encoding="utf-8")
    assert set(up1) >= {"stats", "modules"} and "VIEJO" not in txt1 and "mmorch/route.py" in txt1
    DG.update_readme(p)  # 2da corrida
    assert p.read_text(encoding="utf-8") == txt1  # idempotente
