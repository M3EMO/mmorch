"""Guarda: ningun hook de Claude Code puede llamar a un modulo de mmorch que no existe.

Origen (ticket 02 del mapa de impacto): la poda 19601ef borro mmorch/context_blocks.py por
"sin uso" y dos hooks en ~/.claude/hooks/ lo siguieron ejecutando; fallaron en silencio 10 dias
hasta 19464cb. La arista vive fuera del repo, asi que ni grep ni codegraph la ven."""
import json
import re
from pathlib import Path

import pytest

PKG = Path(__file__).resolve().parents[1] / "mmorch"
_MOD = re.compile(r"mmorch\.([a-z_][a-z0-9_]*)")


def faltantes(claude: Path) -> tuple[set[str], list[str]]:
    """(modulos referenciados por los hooks, los que no existen en mmorch/)."""
    textos = [p.read_text(encoding="utf-8", errors="ignore") for p in (claude / "hooks").glob("*.js")]
    settings = claude / "settings.json"
    if settings.exists():
        textos.append(json.dumps(json.loads(settings.read_text(encoding="utf-8")).get("hooks", {})))
    refs = {m for t in textos for m in _MOD.findall(t)}
    return refs, sorted(m for m in refs if not (PKG / f"{m}.py").exists() and not (PKG / m).is_dir())


def test_hooks_de_esta_maquina_referencian_modulos_existentes():
    refs, faltan = faltantes(Path.home() / ".claude")
    if not refs:
        pytest.skip("sin hooks que llamen a mmorch en esta maquina")
    assert not faltan, f"hooks llaman a modulos de mmorch inexistentes: {faltan}"


def test_la_guarda_detecta_un_hook_roto(tmp_path):
    (tmp_path / "hooks").mkdir()
    (tmp_path / "hooks" / "x.js").write_text('execFileSync(PY, ["-m", "mmorch.no_existe_nunca"])')
    (tmp_path / "settings.json").write_text(json.dumps(
        {"hooks": {"PreToolUse": [{"hooks": [{"command": "python -m mmorch.impacto hook"}]}]}}))
    refs, faltan = faltantes(tmp_path)
    assert refs == {"no_existe_nunca", "impacto"}
    assert faltan == ["no_existe_nunca"]
