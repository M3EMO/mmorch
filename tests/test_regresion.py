"""Tests de la refutacion ejecutable — la tabla de verdad es lo unico que decide."""

import types

from mmorch.regresion import declara_intencion, refutar_ejecutable


def _git(diff="- a\n+ b\n"):
    def fn(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout=diff, stderr="")
    return fn


def _llm(hay=True, test="def test_regresion(): assert True"):
    def fn(prompt, *, schema):
        return {"hay_regresion": hay, "explicacion": "x", "test": test}
    return fn


def _corredor(en_base, en_branch):
    def fn(repo, ref, codigo, **kw):
        return {"paso": en_base if ref == "main" else en_branch, "detalle": ""}
    return fn


def _correr(**kw):
    base = dict(repo="r", branch="b", base="main", git_fn=_git(), llm_fn=_llm())
    return refutar_ejecutable(**{**base, **kw})


def test_solo_pasa_en_base_y_falla_en_branch_es_material():
    assert _correr(correr_fn=_corredor(True, False))["material"] is True


def test_pasa_en_ambos_no_es_regresion():
    assert _correr(correr_fn=_corredor(True, True))["material"] is False


def test_falla_en_ambos_la_objecion_no_se_sostiene():
    """Un test que tampoco pasa con el codigo viejo no describe una regresion:
    describe algo que nunca funciono, o esta mal escrito."""
    r = _correr(correr_fn=_corredor(False, False))
    assert r["material"] is False and "tampoco pasa en base" in r["motivo"]


def test_falla_en_base_y_pasa_en_branch_es_un_arreglo():
    assert _correr(correr_fn=_corredor(False, True))["material"] is False


def test_diferencia_declarada_por_la_branch_no_bloquea():
    """Misma evidencia, lectura distinta: si la branch agrega asserts, el cambio
    de comportamiento es deliberado. Sin esto, TODO arreglo intencional quedaba
    marcado como regresion (medido con 2 de 3 modelos)."""
    r = _correr(git_fn=_git("- a\n+    assert nuevo == 1\n"),
                correr_fn=_corredor(True, False))
    assert r["difiere"] is True and r["intencional"] is True
    assert r["material"] is False and "DECLARADO" in r["motivo"]


def test_sin_regresion_declarada_no_ejecuta_nada():
    def explota(*a, **k):
        raise AssertionError("no debia ejecutar")
    assert _correr(llm_fn=_llm(hay=False), correr_fn=explota)["material"] is False


def test_verificador_caido_es_fail_open():
    def roto(prompt, *, schema):
        raise RuntimeError("sin API")
    def explota(*a, **k):
        raise AssertionError("no debia ejecutar")
    r = _correr(llm_fn=roto, correr_fn=explota)
    assert r["material"] is False and "caido" in r["motivo"]


def test_declara_intencion_solo_cuenta_asserts_AGREGADOS():
    assert declara_intencion("+    assert x == 1\n") is True
    assert declara_intencion("-    assert x == 1\n") is False
    assert declara_intencion("+++ b/tests/test_x.py\n") is False
    assert declara_intencion("     assert x == 1\n") is False
