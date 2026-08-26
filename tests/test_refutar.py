"""Tests del refutador de branches — seams inyectados, cero API."""

import types

from mmorch.refutar import RUBRICA, refutar_branch


def _git(stdout="- viejo\n+ nuevo\n", rc=0):
    def fn(repo, *args):
        return types.SimpleNamespace(returncode=rc, stdout=stdout, stderr="")
    return fn


def _verificador(passed, refutations=()):
    def fn(artifact, **kw):
        return types.SimpleNamespace(passed=passed, confidence=0.9,
                                     refutations=list(refutations),
                                     verifier_model="m", cost_usd=0.0)
    return fn


def test_refuta_cuando_el_verificador_no_pasa():
    r = refutar_branch("r", "b", base="main",
                       verify_fn=_verificador(False, ["el flag miente"]),
                       git_fn=_git())
    assert r["refutado"] and r["razones"] == ["el flag miente"]


def test_no_refuta_cuando_pasa():
    r = refutar_branch("r", "b", base="main", verify_fn=_verificador(True), git_fn=_git())
    assert r["refutado"] is False


def test_verificador_roto_es_fail_open():
    """Un problema de infra JAMAS descarta trabajo: se manda al humano igual."""
    def explota(artifact, **kw):
        raise RuntimeError("sin API")
    r = refutar_branch("r", "b", base="main", verify_fn=explota, git_fn=_git())
    assert r["refutado"] is False and "sin API" in r["error"]


def test_diff_vacio_no_refuta():
    r = refutar_branch("r", "b", base="main",
                       verify_fn=_verificador(False), git_fn=_git(stdout=""))
    assert r["refutado"] is False and "diff vacio" in r["error"]


def test_el_diff_llega_al_verificador_y_la_rubrica_tambien():
    visto = {}

    def espia(artifact, **kw):
        visto["artifact"] = artifact
        visto["rubric"] = kw.get("rubric", "")
        return types.SimpleNamespace(passed=True, confidence=1.0, refutations=[],
                                     verifier_model="m", cost_usd=0.0)

    refutar_branch("r", "b", base="main", verify_fn=espia, git_fn=_git("DIFF-AQUI"))
    assert visto["artifact"] == "DIFF-AQUI" and visto["rubric"] == RUBRICA
