"""Aceptacion A5 de la pieza A: el recall y el MOC nunca muestran una nota reemplazada."""
from __future__ import annotations

import functools
import hashlib

import pytest

import mmorch.memory as M
import mmorch.vault as vault_mod
from mmorch.vault import write_research_note


def _fake_embed(text: str):
    return [float(b) for b in hashlib.sha256(text.encode("utf-8")).digest()[:8]]


@pytest.fixture(params=["sin_embed", "embed_falso"])
def env(request, tmp_path, monkeypatch):
    db = tmp_path / "m.duckdb"
    monkeypatch.setattr(vault_mod, "VAULT", tmp_path / "vault")
    monkeypatch.setattr(M, "remember", functools.partial(M.remember, path=db))
    # peor caso: el gist destilado pierde el path de la nota
    monkeypatch.setattr(M, "distill", lambda t, **kw: t.split(" — ")[0])
    monkeypatch.setattr(M, "embed", (lambda t: None) if request.param == "sin_embed" else _fake_embed)
    return db, tmp_path / "vault"


def _par(vieja, nueva, *, pv, pn, body_v="uno", body_n="dos"):
    pa, _ = write_research_note(vieja, body_v, project=pv)
    pb, _ = write_research_note(nueva, body_n, project=pn, supersedes=pa.stem)
    return pa, pb


def test_pares(env):
    db, vault = env
    olds, news = [], []  # (path, titulo, stem sucesor directo)
    pa, pb = _par("Umbral cascade viejo", "Umbral cascade nuevo", pv="p", pn="p")
    olds.append((pa, "Umbral cascade viejo", pb.stem)); news.append(pb)
    pa, pb = _par("Mismo titulo", "Mismo titulo", pv="p", pn="p", body_v="a", body_n="b")
    assert pb.stem == "mismo-titulo-2"
    olds.append((pa, None, pb.stem)); news.append(pb)  # mismo titulo: se chequea solo el path
    pa, pb = _par("Bandit alfa viejo", "Bandit beta nuevo", pv="alfa", pn="beta")
    olds.append((pa, "Bandit alfa viejo", pb.stem)); news.append(pb)
    a, _ = write_research_note("Cadena A", "a", project="c")
    b, _ = write_research_note("Cadena B", "b", project="c", supersedes=a.stem)
    c, _ = write_research_note("Cadena C", "c", project="c", supersedes=b.stem)
    olds += [(a, "Cadena A", b.stem), (b, "Cadena B", c.stem)]; news.append(c)

    for q in ("umbral cascade", "mismo titulo", "bandit", "cadena", "nota del vault"):
        for fn in (M.recall, M.recall_keyword, M.recall_hybrid):
            texts = " ".join(n.text for n in fn(q, "global", k=50, path=db))
            for old, title, _ in olds:
                assert str(old) not in texts, (fn.__name__, q, old)
                if title:
                    assert title not in texts, (fn.__name__, q, title)
    vivos = " ".join(n.text for n in M.recall_keyword("cadena", "global", k=50, path=db))
    assert "Cadena C" in vivos

    mocs = list((vault / "moc").glob("*.md"))
    assert mocs
    for old, _, succ in olds:
        found = False
        for moc in mocs:
            sec = None
            for ln in moc.read_text(encoding="utf-8").splitlines():
                if ln.startswith("## "):
                    sec = ln
                if f"[[{old.stem}]]" in ln:
                    assert sec == "## Reemplazadas", (moc.name, ln)
                    if ln == f"- [[{old.stem}]] → [[{succ}]]":
                        found = True
        assert found, old.stem


def test_archivada_a_mano_se_oculta_y_reaparece(env):
    db, vault = env
    p, _ = write_research_note("Hallazgo escondible", "x", project="p")
    queda, _ = write_research_note("Hallazgo visible", "y", project="p")

    def textos():
        return {fn.__name__: " ".join(n.text for n in fn("hallazgo", "global", k=50, path=db))
                for fn in (M.recall, M.recall_keyword, M.recall_hybrid)}

    assert "escondible" in textos()["recall_keyword"]
    (vault / "archive").mkdir()
    p.rename(vault / "archive" / p.name)
    for name, t in textos().items():
        assert "escondible" not in t and str(p) not in t, name
    assert "visible" in textos()["recall_keyword"]
    (vault / "archive" / p.name).rename(p)
    for name in ("recall", "recall_keyword"):
        assert "escondible" in textos()[name], name
    assert queda.exists()


def test_fallback_episodico_filtra_antes_del_limit(env, monkeypatch):
    db, vault = env
    monkeypatch.setattr(M, "distill", lambda t, **kw: "SKIP")  # solo episodios, sin gist
    monkeypatch.setattr(M, "embed", lambda t: None)
    viva, _ = write_research_note("Nota viva", "x", project="p")
    muertas = [write_research_note(f"Nota muerta {i}", "x", project="p")[0] for i in range(3)]
    (vault / "archive").mkdir()
    for m in muertas:
        m.rename(vault / "archive" / m.name)
    out = M.recall("nota", "global", k=1, path=db)
    assert len(out) == 1 and str(viva) in out[0].text
