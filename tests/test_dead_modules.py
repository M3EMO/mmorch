"""El parser de imports del ratchet anti-museo.

Existe porque falla en silencio: si deja de reconocer una forma de import, no
tira error — declara vivo un modulo muerto (o peor, muerto uno vivo, que fue el
bug real: un regex que no veia `from .foo import x` ni `from mmorch import a, b`).
"""

import importlib.util
from pathlib import Path

_SRC = Path(__file__).resolve().parent.parent / "tools" / "dead-modules.py"
_spec = importlib.util.spec_from_file_location("dead_modules", _SRC)
dm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(dm)

KNOWN = {"patterns", "checkers", "budget", "canal"}


def _parse(tmp_path, src, name="caller.py"):
    f = tmp_path / name
    f.write_text(src, encoding="utf-8")
    return dm.imported_modules(f, KNOWN)


def test_formas_absolutas(tmp_path):
    src = (
        "import mmorch.patterns\n"
        "from mmorch.checkers import run\n"
        "from mmorch import budget, canal\n"
    )
    assert _parse(tmp_path, src) == {"patterns", "checkers", "budget", "canal"}


def test_formas_relativas_solo_dentro_del_paquete(tmp_path):
    src = "from .patterns import adversarial_verify\nfrom . import budget\nimport checkers\n"
    pkg = dm.PKG / "_tmp_test_caller.py"
    try:
        pkg.write_text(src, encoding="utf-8")
        assert dm.imported_modules(pkg, KNOWN) == {"patterns", "budget", "checkers"}
    finally:
        pkg.unlink(missing_ok=True)
    # el mismo archivo fuera del paquete: los relativos no apuntan a mmorch
    assert _parse(tmp_path, src) == set()


def test_menciones_que_no_son_imports(tmp_path):
    src = '# import patterns\nDOC = "from mmorch.checkers import run"\nbudget = 3\n'
    assert _parse(tmp_path, src) == set()


def test_archivo_roto_no_explota(tmp_path):
    assert _parse(tmp_path, "def (((") == set()


def test_referencia_por_subprocess(tmp_path):
    f = tmp_path / "hook.js"
    f.write_text('spawn("python", ["-m", "mmorch.canal", "tick"])', encoding="utf-8")
    assert dm.referenced_modules(f, KNOWN) == {"canal"}
