"""W6 ronda 2 (fixer) — defectos de verificadores: beat digest sobrevive al distill
roto (D1), secreto ASIGNADO no sale a la API (D2), ingest solo transcripts .jsonl
(D-adv1), exito vacio de reasoning es error explicito (AT-10 #3). D-adv2 (legacy
passed:true refuta) vive en test_patterns. Sin API real."""
import sys, pathlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import pytest

ROOT = pathlib.Path(__file__).resolve().parent.parent


# ---- D1: beat("digest") corre aunque distill falle (AT-19) --------------------
def test_beat_digest_vive_en_finally_despues_del_except():
    """El fix de ronda 1 dejo el beat DENTRO del try: un BreakerOpen en distill
    saltaba al except ANTES del beat -> 'digest: never' cronico. Truth-test de
    estructura (el camino completo necesita LLM): el beat debe vivir en un finally
    POSTERIOR al registro de distill_error."""
    src = (ROOT / "mmorch" / "nightly.py").read_text(encoding="utf-8")
    i_err = src.index('rec["distill_error"]')
    i_beat = src.index('_beat("digest"')
    assert i_beat > i_err, "el beat quedo antes del except: un distill roto lo saltea"
    assert "finally:" in src[i_err:i_beat], "el beat no esta bajo finally"


# ---- D2: clave ASIGNADA sin prefijo conocido no sale a la API -----------------
def test_review_source_rechaza_clave_asignada():
    from mmorch.code_review import review_source
    with pytest.raises(ValueError, match="credential"):
        review_source(code="AWS_SECRET_ACCESS_KEY = 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY'")


def test_review_source_no_bloquea_password_inocuo():
    # anti-falso-rojo: la palabra con valor corto/placeholder sigue siendo revisable
    from mmorch.code_review import review_source
    r = review_source(code="password = 'demo'\nprint(password)\n",
                      find=lambda: [], refute=lambda fs: [])
    assert r["n_raw"] == 0


# ---- D-adv1: ingest_session solo acepta transcripts .jsonl --------------------
def test_ingest_session_rechaza_path_no_jsonl(tmp_path):
    from mmorch.sessions import ingest_session
    p = tmp_path / "system.ini"
    p.write_text("[boot]\nshell=explorer.exe\n", encoding="utf-8")
    with pytest.raises(ValueError, match="jsonl"):
        ingest_session(str(p), recorder=lambda **k: None,
                       classifier=lambda req, **k: type("R", (), {"domain": "clear"})(),
                       ledger=tmp_path / "l.txt")


# ---- AT-10 #3: text='' con budget agotado en reasoning = error, no exito ------
class _Usage:
    prompt_tokens = 5
    completion_tokens = 5


def _resp(text, finish=None):
    ch = types.SimpleNamespace(message=types.SimpleNamespace(content=text))
    if finish is not None:
        ch.finish_reason = finish
    return types.SimpleNamespace(choices=[ch], usage=_Usage())


def _client(resp):
    return types.SimpleNamespace(chat=types.SimpleNamespace(
        completions=types.SimpleNamespace(create=lambda **kw: resp)))


def test_call_respuesta_vacia_por_length_levanta(monkeypatch):
    import mmorch.providers as PV
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: _client(_resp("", finish="length")))
    with pytest.raises(RuntimeError, match="max_tokens"):
        PV.call("glm-5.2", "hola", max_tokens=5)


def test_call_texto_normal_con_finish_length_pasa(monkeypatch):
    # truncado con contenido NO es exito vacio: el caller decide que hacer
    import mmorch.providers as PV
    monkeypatch.setattr(PV, "log_event", lambda **rec: None)
    monkeypatch.setattr(PV, "_client", lambda mk: _client(_resp("parcial", finish="length")))
    assert PV.call("glm-5.2", "hola").text == "parcial"
