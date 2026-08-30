"""docgen: catalogo generado + ratchet anti-copia. Idempotente."""
import sys
import pathlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.docgen as DG


def test_stats_counts_real_package():
    s = DG.stats()
    assert s["modules"] >= 10 and s["tools"] >= 5 and s["tests"] >= 20


def test_module_table_includes_known_modules():
    t = DG.module_table()
    assert "mmorch/route.py" in t and "mmorch/cascade.py" in t and "mmorch/patterns.py" in t
    assert "docgen.py" not in t


def test_mcp_tools_introspected():
    ts = DG.mcp_tools()
    assert "mmorch_fan_out" in ts and "mmorch_cascade" in ts


def test_update_readme_idempotent_and_points_at_catalog(tmp_path):
    p = tmp_path / "R.md"
    p.write_text(
        "# x\n<!-- mmorch:auto:stats -->\nVIEJO\n<!-- /mmorch:auto:stats -->\n"
        "<!-- mmorch:auto:modules -->\n<!-- /mmorch:auto:modules -->\n", encoding="utf-8")
    up1 = DG.update_readme(p)
    txt1 = p.read_text(encoding="utf-8")
    assert set(up1) >= {"stats", "modules"} and "VIEJO" not in txt1
    assert "docs/generated/catalog.md" in txt1
    assert "mmorch/route.py" not in txt1  # la tabla ya no vive en el README
    DG.update_readme(p)
    assert p.read_text(encoding="utf-8") == txt1


def test_catalog_markdown_has_modules_and_tools():
    md = DG.catalog_markdown()
    assert "mmorch/route.py" in md and "mmorch_fan_out" in md
    assert "Do not edit" in md


def test_contract_hits_catches_registry_and_tools():
    keys = ["deepseek-chat", "gemini-3.1-flash-lite"]
    hits = DG.contract_hits(
        "use deepseek-chat then mmorch_fan_out; 12 tests and 3 tools here",
        keys)
    assert any("deepseek-chat" in h for h in hits)
    assert any("mmorch_fan_out" in h for h in hits)
    assert any("test-count" in h for h in hits)
    assert any("tool-count" in h for h in hits)


def test_contract_hits_ignores_beads_block():
    keys = ["deepseek-chat"]
    text = (
        "ok pointer to config.py\n"
        "<!-- BEGIN BEADS INTEGRATION v:1 -->\n"
        "call mmorch_remember and deepseek-chat\n"
        "<!-- END BEADS INTEGRATION -->\n"
    )
    assert DG.contract_hits(text, keys) == []


def test_ssot_repo_is_clean():
    assert DG.ssot_violations() == [], DG.ssot_violations()
