"""Modulo supersedes del vault: validacion, retiro atomico y ediciones a mano."""
from __future__ import annotations

import pytest

import mmorch.vault as vault_mod
from mmorch import config
from mmorch.vault import regenerate_moc, write_validated

VIEJA = "---\ntitle: Vieja\ntags:\n- p\n- otra\n---\ncuerpo viejo\n"


@pytest.fixture()
def vault(tmp_path, monkeypatch):
    v = tmp_path / "v"
    (v / "research").mkdir(parents=True)
    monkeypatch.setattr(vault_mod, "VAULT", v)
    return v


def _raw(vault, stem, text, folder="research"):
    p = vault / folder / f"{stem}.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


def _fakes():
    calls = []
    return calls, (lambda gist, **kw: calls.append(gist))


def _nada_escrito(vault, calls, live=("vieja",)):
    assert sorted(p.stem for p in (vault / "research").glob("*.md")) == sorted(live)
    assert not (vault / "moc").exists() and not (vault / "log.md").exists() and not calls


def test_inexistente_no_escribe_nada(vault):
    _raw(vault, "vieja", VIEJA)
    calls, rem = _fakes()
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", supersedes=["vieja", "nada"], remember_fn=rem)
    _nada_escrito(vault, calls)


@pytest.mark.parametrize("name", ["../x", "a/b", "..", "", "."])
def test_rechaza_traversal(vault, name):
    fuera = vault.parent / "x.md"
    fuera.write_text("fuera", encoding="utf-8")
    calls, rem = _fakes()
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", supersedes=[name], remember_fn=rem)
    assert fuera.read_text(encoding="utf-8") == "fuera"
    _nada_escrito(vault, calls, live=())


def test_modulo_apagado_rechaza(vault, monkeypatch):
    monkeypatch.setattr(config, "VAULT_MODULES", frozenset({"code"}))
    _raw(vault, "vieja", VIEJA)
    calls, rem = _fakes()
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", supersedes=["vieja"], remember_fn=rem)
    _nada_escrito(vault, calls)


def test_mueve_marca_y_avisa(vault):
    _raw(vault, "vieja", VIEJA)
    calls, rem = _fakes()
    p = write_validated("Nueva", "cuerpo nuevo", project="p", supersedes=["vieja"], remember_fn=rem)
    assert not (vault / "research" / "vieja.md").exists()
    txt = (vault / "archive" / "vieja.md").read_text(encoding="utf-8")
    assert "tags:\n- p\n- otra\n" in txt, "la lista en bloque sobrevive"
    assert "superseded_by: nueva\n---\nReemplazada por [[nueva]]\n" in txt
    assert txt.endswith("cuerpo viejo\n")
    assert "supersedes: [vieja]" in p.read_text(encoding="utf-8")
    assert len(calls) == 1
    assert "[[vieja]] → [[nueva]]" in (vault / "moc" / "p.md").read_text(encoding="utf-8")


def test_destino_ocupado_no_escribe(vault):
    _raw(vault, "vieja", VIEJA)
    _raw(vault, "vieja", "previo", folder="archive")
    calls, rem = _fakes()
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", supersedes=["vieja"], remember_fn=rem)
    _nada_escrito(vault, calls)
    assert (vault / "archive" / "vieja.md").read_text(encoding="utf-8") == "previo"


def test_completa_supersedes_a_mano(vault):
    _raw(vault, "vieja", VIEJA)
    _raw(vault, "nueva", "---\ntitle: Nueva\ntags: [p]\nsupersedes: [vieja]\n---\nx\n")
    moc = regenerate_moc("p").read_text(encoding="utf-8")
    assert not (vault / "research" / "vieja.md").exists()
    assert "superseded_by: nueva" in (vault / "archive" / "vieja.md").read_text(encoding="utf-8")
    assert "- [[vieja]] → [[nueva]]" in moc


def test_completa_superseded_by_a_mano(vault):
    _raw(vault, "vieja", "---\ntitle: Vieja\ntags: [p]\nsuperseded_by: nueva\n---\ncuerpo\n")
    nueva = _raw(vault, "nueva", "---\ntitle: Nueva\ntags: [p]\n---\nx\n")
    antes = nueva.read_text(encoding="utf-8")
    moc = regenerate_moc("p").read_text(encoding="utf-8")
    txt = (vault / "archive" / "vieja.md").read_text(encoding="utf-8")
    assert "Reemplazada por [[nueva]]" in txt and txt.count("superseded_by") == 1
    assert nueva.read_text(encoding="utf-8") == antes, "P4: no escribe supersedes"
    assert "- [[vieja]] → [[nueva]]" in moc


def test_idempotencia(vault):
    _raw(vault, "vieja", VIEJA)
    _raw(vault, "nueva", "---\ntitle: Nueva\ntags: [p]\nsupersedes: [vieja]\n---\nx\n")
    m1 = regenerate_moc("p").read_bytes()
    a1 = (vault / "archive" / "vieja.md").read_bytes()
    assert regenerate_moc("p").read_bytes() == m1
    vault_mod._retire(vault / "archive" / "vieja.md", "nueva")
    assert (vault / "archive" / "vieja.md").read_bytes() == a1


def test_supersedes_roto_no_rompe_la_regeneracion(vault):
    _raw(vault, "nueva", "---\ntitle: Nueva\ntags: [p]\nsupersedes: [fantasma]\n---\nx\n")
    moc = regenerate_moc("p").read_text(encoding="utf-8")
    assert "[[nueva]]" in moc
    assert "fantasma" in (vault / "log.md").read_text(encoding="utf-8")


def test_apagado_no_completa_a_mano(vault, monkeypatch):
    monkeypatch.setattr(config, "VAULT_MODULES", frozenset({"code"}))
    _raw(vault, "vieja", VIEJA)
    _raw(vault, "nueva", "---\ntitle: Nueva\ntags: [p]\nsupersedes: [vieja]\n---\nx\n")
    regenerate_moc("p")
    assert (vault / "research" / "vieja.md").exists()


def test_superseded_by_a_mano_sin_sucesora_no_mueve(vault):
    p = _raw(vault, "vieja", "---\ntitle: Vieja\ntags: [p]\nsuperseded_by: fantasma\n---\nx\n")
    antes = p.read_bytes()
    regenerate_moc("p")
    assert p.read_bytes() == antes and not (vault / "archive").exists()
    assert "fantasma" in (vault / "log.md").read_text(encoding="utf-8")


def test_superseded_by_a_mano_con_sucesora_archivada_si_mueve(vault):
    _raw(vault, "vieja", "---\ntitle: V\ntags: [p]\nsuperseded_by: nueva\n---\nx\n")
    _raw(vault, "nueva", "---\ntitle: N\ntags: [p]\n---\nx\n", folder="archive")
    regenerate_moc("p")
    assert (vault / "archive" / "vieja.md").exists()


def test_vieja_de_otro_proyecto_regenera_su_moc(vault):
    _raw(vault, "vieja", "---\ntitle: V\ntags: [q]\n---\nx\n")
    _raw(vault, "nueva", "---\ntitle: N\ntags: [p]\nsupersedes: [vieja]\n---\nx\n")
    regenerate_moc("q")  # q ya tiene MOC, con la vieja viva
    assert "[[vieja]]" in (vault / "moc" / "q.md").read_text(encoding="utf-8")
    regenerate_moc("p")
    moc_q = (vault / "moc" / "q.md").read_text(encoding="utf-8")
    assert moc_q.count("[[vieja]]") == 1
    assert moc_q.split("## Reemplazadas")[1].count("[[vieja]]") == 1


def test_superseded_by_en_lista_en_bloque_no_rompe(vault):
    _raw(vault, "arch", "---\ntitle: A\ntags: [p]\nsuperseded_by:\n- nueva\n---\nx\n",
         folder="archive")
    _raw(vault, "viva", "---\ntitle: V\ntags: [p]\nsuperseded_by:\n- nueva\n---\nx\n")
    _raw(vault, "nueva", "---\ntitle: N\ntags: [p]\n---\nx\n")
    moc = regenerate_moc("p").read_text(encoding="utf-8")
    assert "- [[arch]] → [[nueva]]" in moc and "- [[viva]] → [[nueva]]" in moc
    assert (vault / "archive" / "viva.md").exists()


def test_symlink_no_se_resuelve_ni_se_mueve(vault):
    fuera = vault.parent / "fuera.md"
    fuera.write_text("fuera", encoding="utf-8")
    (vault / "research" / "vieja.md").symlink_to(fuera)
    calls, rem = _fakes()
    with pytest.raises(ValueError):
        write_validated("Nueva", "b", project="p", supersedes=["vieja"], remember_fn=rem)
    assert fuera.read_text(encoding="utf-8") == "fuera" and not calls
    assert (vault / "research" / "vieja.md").is_symlink()
