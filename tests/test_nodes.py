"""tests nodes.py — el registry de la orquesta."""
from mmorch.nodes import orchestra, members, sections, conductor, Node


def test_conductor_not_musician():
    c = conductor()
    assert c["es_musico"] is False and "mmorch" in c["name"]


def test_roster_has_all_sections():
    s = sections()
    for sec in ("generator", "verifier", "router", "soloist", "memory"):
        assert s.get(sec, 0) > 0


def test_handles_unique_and_prefixed():
    hs = [n.handle for n in orchestra()]
    assert len(hs) == len(set(hs))                       # sin duplicados
    assert all(":" in h for h in hs)                     # convencion seccion:nombre


def test_checkers_are_verifiers():
    chk = [n for n in members("verifier") if n.kind == "checker"]
    assert len(chk) >= 15 and all(n.handle.startswith("check:") for n in chk)


def test_planned_includes_synth():
    planned = {n.handle for n in orchestra() if n.status == "planned"}
    assert "gen:synth" in planned


def test_code_embedder_promoted_active():
    # flywheel lo promovio: bate a bge en code-quality, inferencia numpy pura
    active = {n.handle for n in orchestra() if n.status == "active"}
    assert "model:code_embedder" in active
