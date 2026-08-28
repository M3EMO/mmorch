"""Tests del triage mecanico de branches (cero LLM, cero git real salvo el seam)."""

import types

from mmorch.triage import triage_archivo, triage_branch

BASE = "def f(x):\n    # el porque de este borde, medido\n    return x + 1\n"


def test_demo_self_check_pasa():
    from mmorch.triage import _demo
    _demo()


def test_solo_espacios_es_ruido():
    assert triage_archivo(BASE, BASE.rstrip("\n"))[0] == "descartar"


def test_reemplazar_un_comentario_no_pasa_invisible():
    """El caso real de health.py: saca 1 comentario y pone 1, neto 0."""
    otro = "def f(x):\n    # explicacion distinta\n    return x + 2\n"
    v, m = triage_archivo(BASE, otro)
    assert v == "revisar" and "pisa 1" in m


def test_cambio_real_que_conserva_el_porque_pasa():
    assert triage_archivo(BASE, BASE.replace("x + 1", "x + 2"))[0] == "ok"


def test_fuente_no_parseable_nunca_se_descarta_sola():
    """Sin poder parsear no se puede AFIRMAR 'no cambia el comportamiento', asi
    que jamas cae en 'descartar': queda para el humano."""
    assert triage_archivo(BASE, "def f(:\n")[0] == "revisar"


def _git_falso(archivos: dict[str, tuple[str, str]]):
    """git_fn inyectado: {path: (antes, despues)}."""
    def fn(repo, *args):
        if args[0] == "diff":
            return types.SimpleNamespace(returncode=0, stdout="\n".join(archivos), stderr="")
        if args[0] == "show":
            ref, path = args[1].split(":", 1)
            antes, despues = archivos[path]
            return types.SimpleNamespace(
                returncode=0, stdout=antes if ref == "main" else despues, stderr="")
        raise AssertionError(args)
    return fn


def test_branch_toma_el_peor_veredicto_de_sus_archivos():
    git = _git_falso({"mmorch/a.py": (BASE, BASE.replace("x + 1", "x + 2")),   # ok
                      "mmorch/b.py": (BASE, BASE.rstrip("\n"))})               # descartar
    r = triage_branch("r", "rama", base="main", git_fn=git)
    assert r["veredicto"] == "descartar"
    assert r["archivos"]["mmorch/a.py"][0] == "ok"


def test_branch_sin_archivos_py_va_a_revision_humana():
    def git(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout="README.md\n", stderr="")
    assert triage_branch("r", "rama", base="main", git_fn=git)["veredicto"] == "revisar"
