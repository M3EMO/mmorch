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


def test_bridge_remember_y_cola_babel(tmp_vault):
    calls, rem, enq = _fakes()
    p = write_validated("Nota tres", "b" * 50, project="mmorch",
                        remember_fn=rem, enqueue_babel_fn=enq)
    assert len(calls["remember"]) == 1, "gist no bridgeado a memoria"
    gist = calls["remember"][0][0]
    assert str(p) in gist or p.name in gist, "el gist no referencia el path de la nota"
    assert calls["babel"] == [p], "job babel no encolado con el path"


def test_fallo_de_side_channels_no_rompe_el_write(tmp_vault):
    def boom(*a, **kw):
        raise RuntimeError("side channel roto")
    p = write_validated("Nota cuatro", "cuerpo", project="mmorch",
                        remember_fn=boom, enqueue_babel_fn=boom)
    assert p.exists(), "la nota debe escribirse aunque bridge/cola fallen"
