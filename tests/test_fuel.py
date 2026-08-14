"""Tests for fuel module."""

from pathlib import Path

import pytest

from fuel import (
    detect_promotions,
    expire,
    generate,
    has_new_fuel,
    parse,
    render,
)


class FakePropose:
    """Fake propose API."""

    def __init__(self, today):
        self.today = today
        self.added = []

    def add(self, text, lente, ya_visto, gist=None):
        self.added.append((text, lente, ya_visto, gist))
        return len(self.added)


class FakeRefute:
    """Fake refute API."""

    def __init__(self):
        self.refuted = set()

    def is_refuted(self, text):
        return text in self.refuted


@pytest.fixture
def tmp_path():
    """Temporary directory fixture."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        yield Path(tmp)


@pytest.fixture
def propose():
    """Fake propose fixture."""
    return FakePropose("2024-01-15")


@pytest.fixture
def refute():
    """Fake refute fixture."""
    return FakeRefute()


SEEDED_ENTRY = """- [ ] #fuel 2024-01-15
  - lente: "lente-a"
  - texto: "Candidato de prueba con formato real"
  - gist: "abc123"
"""


def test_parse_real_seeded_format(tmp_path):
    """Parse a real seeded format entry."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(SEEDED_ENTRY)
    fuels = parse(file_path)
    assert len(fuels) == 1
    fuel = fuels[0]
    assert fuel.lente == "lente-a"
    assert fuel.texto == "Candidato de prueba con formato real"
    assert fuel.gist == "abc123"
    assert fuel.fecha == "2024-01-15"


def test_roundtrip_parse_render(tmp_path):
    """Roundtrip parse(render(x)) preserves data."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(SEEDED_ENTRY)
    fuels = parse(file_path)
    rendered = render(fuels)
    reparsed = parse(rendered)
    assert len(reparsed) == 1
    assert reparsed[0].lente == fuels[0].lente
    assert reparsed[0].texto == fuels[0].texto
    assert reparsed[0].gist == fuels[0].gist
    assert reparsed[0].fecha == fuels[0].fecha


def test_has_new_fuel_true(tmp_path):
    """has_new_fuel returns True when file has new fuel."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(SEEDED_ENTRY)
    assert has_new_fuel(file_path) is True


def test_has_new_fuel_false(tmp_path):
    """has_new_fuel returns False when file has no new fuel."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text("- [x] #fuel 2024-01-15\n")
    assert has_new_fuel(file_path) is False


def test_has_new_fuel_nonexistent(tmp_path):
    """has_new_fuel returns False for nonexistent path."""
    file_path = tmp_path / "no_existe.md"
    assert has_new_fuel(file_path) is False


def test_generate_adds_with_different_lentes(tmp_path, propose, refute):
    """generate adds entries with different lentes."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato uno"\n'
        '  - gist: "gist1"\n'
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-b"\n'
        '  - texto: "Candidato dos"\n'
        '  - gist: "gist2"\n'
    )
    generate(file_path, propose, refute, today="2024-01-15")
    assert len(propose.added) == 2
    assert propose.added[0][1] == "lente-a"
    assert propose.added[1][1] == "lente-b"


def test_generate_dedup_against_ya_visto(tmp_path, propose, refute):
    """generate deduplicates against ya_visto."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato repetido"\n'
        '  - gist: "gist1"\n'
    )
    ya_visto = {"Candidato repetido"}
    generate(file_path, propose, refute, today="2024-01-15", ya_visto=ya_visto)
    assert len(propose.added) == 0


def test_generate_refuted_does_not_enter(tmp_path, propose, refute):
    """generate skips refuted entries."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato refutado"\n'
        '  - gist: "gist1"\n'
    )
    refute.refuted.add("Candidato refutado")
    generate(file_path, propose, refute, today="2024-01-15")
    assert len(propose.added) == 0


def test_generate_gist_none_produces_nothing(tmp_path, propose, refute):
    """generate skips entries with gist None."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato sin gist"\n'
        "  - gist: None\n"
    )
    generate(file_path, propose, refute, today="2024-01-15")
    assert len(propose.added) == 0


def test_generate_sequential_ids_same_day(tmp_path, propose, refute):
    """generate produces sequential ids for same day."""
    file_path = tmp_path / "candidatos.md"
    file_path.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato uno"\n'
        '  - gist: "gist1"\n'
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-b"\n'
        '  - texto: "Candidato dos"\n'
        '  - gist: "gist2"\n'
    )
    generate(file_path, propose, refute, today="2024-01-15")
    assert propose.added[0][0] == "2024-01-15-1"
    assert propose.added[1][0] == "2024-01-15-2"


def test_expire_moves_to_archivadas(tmp_path):
    """expire moves old fuels to Archivadas with reward 0.2."""
    candidatos = tmp_path / "candidatos.md"
    archivadas = tmp_path / "archivadas.md"
    candidatos.write_text(
        "- [ ] #fuel 2024-01-01\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato viejo"\n'
        '  - gist: "gist1"\n'
    )
    archivadas.write_text("")
    expire(candidatos, archivadas, today="2024-01-15")
    archivadas_content = archivadas.read_text()
    assert "Candidato viejo" in archivadas_content
    assert "reward: 0.2" in archivadas_content
    assert "lente-a" in archivadas_content
    assert candidatos.read_text() == ""


def test_detect_promotions_moves_with_1_0(tmp_path):
    """detect_promotions moves entries with 1.0 reward."""
    candidatos = tmp_path / "candidatos.md"
    archivadas = tmp_path / "archivadas.md"
    candidatos.write_text(
        "- [ ] #fuel 2024-01-15\n"
        '  - lente: "lente-a"\n'
        '  - texto: "Candidato promovido"\n'
        '  - gist: "gist1"\n'
        "  - reward: 1.0\n"
    )
    archivadas.write_text("")
    detect_promotions(candidatos, archivadas)
    archivadas_content = archivadas.read_text()
    assert "Candidato promovido" in archivadas_content
    assert "reward: 1.0" in archivadas_content
    assert candidatos.read_text() == ""


def test_nonexistent_roadmap_does_not_break(tmp_path, propose, refute):
    """generate handles nonexistent roadmap gracefully."""
    file_path = tmp_path / "no_existe.md"
    generate(file_path, propose, refute, today="2024-01-15")
    assert len(propose.added) == 0
