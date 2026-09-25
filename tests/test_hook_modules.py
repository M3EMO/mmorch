"""Los hooks de Claude Code que corren `-m mmorch.X` apuntan a modulos que existen.

Existe porque la arista hook -> modulo vive fuera del repo: ni codegraph ni el grep
de una poda la ven. La poda 19601ef borro context_blocks y dos hooks fallaron en
silencio (fail open) durante 10 dias.
"""

import re
from pathlib import Path

import pytest

_REPO = Path(__file__).resolve().parent.parent
_CLAUDE = Path.home() / ".claude"
_MOD = re.compile(r"""-m["',\s]+(mmorch(?:\.\w+)+)""")


def _sources():
    return sorted((_CLAUDE / "hooks").glob("*.js")) + [
        p for p in (_CLAUDE / "settings.json", _CLAUDE / "settings.local.json") if p.exists()
    ]


def _exists(mod):
    p = _REPO.joinpath(*mod.split("."))
    return p.with_suffix(".py").exists() or (p / "__init__.py").exists()


def test_regex_ve_las_formas_de_invocacion():
    assert _MOD.findall('execFileSync(PY, ["-m", "mmorch.context_blocks", "latest"])') == ["mmorch.context_blocks"]
    assert _MOD.findall("python -m mmorch.a.b tick") == ["mmorch.a.b"]


@pytest.mark.skipif(not (_CLAUDE / "hooks").is_dir(), reason="sin ~/.claude/hooks en esta maquina")
def test_hooks_no_apuntan_a_modulos_borrados():
    rotos = [
        f"{src.name}: {mod}"
        for src in _sources()
        for mod in _MOD.findall(src.read_text(encoding="utf-8", errors="replace"))
        if not _exists(mod)
    ]
    assert not rotos, "hooks que ejecutan modulos mmorch inexistentes:\n" + "\n".join(rotos)
