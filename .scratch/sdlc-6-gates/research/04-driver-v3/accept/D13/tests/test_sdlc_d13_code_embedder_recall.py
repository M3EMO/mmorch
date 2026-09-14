"""D13 (cableo aprobado 2026-09-14): `code_embedder` entra a `memory` para notas de CODIGO, con pesos
verificados por `weights` (sha256 del manifest) antes de inferir.

Contrato:
- R1 `code_embedder._load()` resuelve el .npz con `weights.resolve("code_embedder", verify_hash=True)`;
  si `weights.verify` falla, `available()` es False y `embed_code()` devuelve None (nunca infiere con pesos dudosos).
- R2 `write_note(..., kind="code")` embebe con code_embedder y guarda emb_model="code_embedder", dim=384.
- R3 `recall(query, kind="code")` embebe la query con code_embedder y compara SOLO contra notas con
  emb_model="code_embedder" (espacios distintos no se mezclan).
- R4 `kind` default "text": comportamiento de hoy (bge o embedding NULL); recall de texto ignora notas de codigo.
Cero red: fastembed puede faltar (embedding NULL) sin romper los tests.
"""
import pytest

import mmorch.code_embedder as CE
import mmorch.memory as M
import mmorch.weights as W

CODE_A = "def parse_lines(lines):\n    return [l.split(',', 2) for l in lines]\n"
CODE_B = "class TokenBucket:\n    def allow(self, now):\n        return True\n"
TEXT = "el bandit aprende el umbral de la cascada con outcomes reales"


def _emb_model(db, note_id):
    con = M._connect(db)
    try:
        return con.execute("SELECT emb_model, dim FROM semantic WHERE id = ?", [note_id]).fetchone()
    finally:
        con.close()


def test_R1_pesos_no_verificados_apagan_el_encoder(monkeypatch):
    monkeypatch.setattr(W, "verify", lambda name, **kw: (False, "sha256 distinto"))
    monkeypatch.setattr(CE, "_CACHE", None, raising=False)
    assert CE.available() is False
    assert CE.embed_code(CODE_A) is None


@pytest.mark.skipif(not CE.available(), reason="pesos del encoder ausentes")
def test_R2_nota_de_codigo_usa_code_embedder(tmp_path):
    db = tmp_path / "mem.duckdb"
    nid = M.write_note("proj", CODE_A, kind="code", path=db)
    model, dim = _emb_model(db, nid)
    assert model == "code_embedder" and dim == 384, (model, dim)


@pytest.mark.skipif(not CE.available(), reason="pesos del encoder ausentes")
def test_R3_recall_de_codigo_solo_compara_con_codigo(tmp_path):
    db = tmp_path / "mem.duckdb"
    a = M.write_note("proj", CODE_A, kind="code", path=db)
    M.write_note("proj", CODE_B, kind="code", path=db)
    M.write_note("proj", TEXT, path=db)
    got = M.recall("def parse_lines(lines)", "proj", kind="code", k=2, track=False, path=db)
    assert got and got[0].id == a, got
    assert all(n.text != TEXT for n in got)


def test_R4_texto_no_toca_el_encoder(tmp_path):
    db = tmp_path / "mem.duckdb"
    nid = M.write_note("proj", TEXT, path=db)
    model, _ = _emb_model(db, nid)
    assert model != "code_embedder"
    got = M.recall("bandit umbral cascada", "proj", k=3, track=False, path=db)
    assert all(n.id == nid for n in got)
