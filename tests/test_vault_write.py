"""Acceptance test (seam 1) de la spec vault-global: contrato de vault.write_validated.

Escrito ANTES del build (gate independiente — el engine implementa hasta que esto
pase). Decisiones fuente: .scratch/vault-global/spec.md tickets 03/04/09.
"""
from __future__ import annotations

import re

import pytest

import mmorch.vault as vault_mod
from mmorch.vault import write_validated


@pytest.fixture()
def tmp_vault(tmp_path, monkeypatch):
    monkeypatch.setattr(vault_mod, "VAULT", tmp_path)
    return tmp_path


def _fakes():
    calls = {"remember": [], "babel": []}
    return calls, (lambda gist, **kw: calls["remember"].append((gist, kw))), \
        (lambda path: calls["babel"].append(path))


def test_valida_frontmatter_minimo_duro(tmp_vault):
    calls, rem, enq = _fakes()
    with pytest.raises(ValueError):
        write_validated("", "cuerpo", project="mmorch",
                        remember_fn=rem, enqueue_babel_fn=enq)
    with pytest.raises(ValueError):
        write_validated("titulo", "cuerpo", project="",
                        remember_fn=rem, enqueue_babel_fn=enq)
    assert not calls["remember"] and not calls["babel"]


def test_escribe_nota_con_created_auto_y_tag_proyecto(tmp_vault):
    _, rem, enq = _fakes()
    p = write_validated("Mi nota de research", "hallazgo X medido", project="lotus",
                        folder="research", remember_fn=rem, enqueue_babel_fn=enq)
    txt = p.read_text(encoding="utf-8")
    assert p.parent.name == "research" and p.suffix == ".md"
    assert re.search(r"created: \d{4}-\d{2}-\d{2}", txt), "created no autocompletado"
    assert "lotus" in txt.split("---")[1], "tag de proyecto ausente en frontmatter"
    assert "hallazgo X medido" in txt


def test_regenera_moc_del_proyecto(tmp_vault):
    _, rem, enq = _fakes()
    write_validated("Nota uno", "cuerpo", project="mmorch",
                    remember_fn=rem, enqueue_babel_fn=enq)
    moc = tmp_vault / "moc" / "mmorch.md"
    assert moc.exists(), "MOC del proyecto no regenerado al escribir"
    assert "[[nota-uno]]" in moc.read_text(encoding="utf-8")


def test_moc_excluye_infra_y_archive(tmp_vault):
    _, rem, enq = _fakes()
    (tmp_vault / "archive").mkdir(parents=True)
    (tmp_vault / "archive" / "viejo.md").write_text(
        "---\ntitle: viejo\ntags: [mmorch]\n---\nx", encoding="utf-8")
    (tmp_vault / "lexicon.md").write_text("# lexicon", encoding="utf-8")
    write_validated("Nota dos", "cuerpo", project="mmorch",
                    remember_fn=rem, enqueue_babel_fn=enq)
    moc = (tmp_vault / "moc" / "mmorch.md").read_text(encoding="utf-8")
    assert "viejo" not in moc and "lexicon" not in moc


def test_moc_incluye_tags_inline_y_applies_to_en_bloque(tmp_vault):
    """2026-10-01: regenerate_moc solo leia `tags` inline y borraba del MOC las notas que
    declaran el proyecto en `applies_to` como lista YAML en bloque."""
    research = tmp_vault / "research"
    research.mkdir()
    (research / "inline.md").write_text(
        "---\ntitle: inline\ntags: [research, orchestration]\n---\nx", encoding="utf-8")
    (research / "bloque.md").write_text(
        "---\ntitle: bloque\napplies_to:\n- orchestration\n- .claude\n"
        "status: applied\nconfidence: 0.9\n---\nx", encoding="utf-8")
    (research / "parecido.md").write_text(
        "---\ntitle: parecido\napplies_to:\n- orchestration-old\n---\nx", encoding="utf-8")
    moc = vault_mod.regenerate_moc("orchestration").read_text(encoding="utf-8")
    assert "[[inline]]" in moc
    assert "- [[bloque]] — applied · conf 0.9" in moc
    assert "parecido" not in moc, "membresia exacta: 'orchestration-old' no es el proyecto"


def test_frontmatter_de_write_note_es_yaml_valido(tmp_vault):
    """2026-10-02: write_note escribia 'title: a: b' y URLs con '?' en [..] sin comillas;
    el YAML invalido hacia que adjudicate borrara el frontmatter entero."""
    yaml = pytest.importorskip("yaml")
    title = "Decision systems: majority voting — panel"
    sources = ["https://openreview.net/pdf?id=qY", "docs/rlm.md", "a, b"]
    p = vault_mod.write_note("research", title, "cuerpo", frontmatter={
        "tags": ["research", "orchestration"], "status": "applied",
        "confidence": "alta: medido", "sources": sources})
    fm = yaml.safe_load(p.read_text(encoding="utf-8").split("---")[1])
    assert fm["title"] == title and fm["sources"] == sources
    assert fm["tags"] == ["research", "orchestration"]
    moc = vault_mod.regenerate_moc("orchestration").read_text(encoding="utf-8")
    assert "— applied · conf alta: medido" in moc, "el MOC debe mostrar el valor sin comillas"


def test_bridge_remember_y_cola_babel(tmp_vault):
    calls, rem, enq = _fakes()
    p = write_validated("Nota tres", "b" * 50, project="mmorch",
                        remember_fn=rem, enqueue_babel_fn=enq)
    assert len(calls["remember"]) == 1, "gist no bridgeado a memoria"
    gist = calls["remember"][0][0]
    assert str(p) in gist or p.name in gist, "el gist no referencia el path de la nota"
    assert calls["babel"] == [p], "job babel no encolado con el path"


def test_log_de_operaciones_parseable(tmp_vault):
    _, rem, enq = _fakes()
    write_validated("Nota logueada", "cuerpo", project="mmorch",
                    remember_fn=rem, enqueue_babel_fn=enq)
    log = (tmp_vault / "log.md").read_text(encoding="utf-8")
    assert re.search(r"^## \[\d{4}-\d{2}-\d{2}\] write \| Nota logueada",
                     log, re.M), "entrada de log parseable ausente"


def test_fallo_de_side_channels_no_rompe_el_write(tmp_vault):
    def boom(*a, **kw):
        raise RuntimeError("side channel roto")
    p = write_validated("Nota cuatro", "cuerpo", project="mmorch",
                        remember_fn=boom, enqueue_babel_fn=boom)
    assert p.exists(), "la nota debe escribirse aunque bridge/cola fallen"


def test_colision_de_slug_no_pisa_la_nota_vieja(tmp_vault):
    """audit 2026-08 #11: dos titulos que colisionan en el slug (o un titulo re-usado con
    otro cuerpo) no deben perder la nota anterior en silencio."""
    p1 = vault_mod.write_note("research", "Mismo Titulo", "contenido original")
    p2 = vault_mod.write_note("research", "Mismo Titulo", "contenido DISTINTO")
    assert p1 != p2, "la segunda escritura debe ir a un path distinto, no pisar la primera"
    assert p1.read_text(encoding="utf-8").endswith("contenido original\n")
    assert "contenido DISTINTO" in p2.read_text(encoding="utf-8")


def test_mismo_titulo_mismo_contenido_reusa_el_path(tmp_vault):
    """Idempotencia real (no es colision): re-escribir con el MISMO contenido reusa el
    mismo path en vez de crear -2, -3, ... indefinidamente."""
    p1 = vault_mod.write_note("research", "Titulo Estable", "cuerpo\n")
    p2 = vault_mod.write_note("research", "Titulo Estable", "cuerpo\n")
    assert p1 == p2
