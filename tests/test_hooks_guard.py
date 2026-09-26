"""Guarda: ningun hook de Claude Code o Cursor puede llamar a un modulo de mmorch, o a un script
de orchestration/scripts/, que no existe.

Origen (ticket 02 del mapa de impacto): la poda 19601ef borro mmorch/context_blocks.py por
"sin uso" y dos hooks en ~/.claude/hooks/ lo siguieron ejecutando; fallaron en silencio 10 dias
hasta 19464cb. La arista vive fuera del repo, asi que ni grep ni codegraph la ven."""
import json
import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]
PKG = REPO / "mmorch"
_MOD = re.compile(r"mmorch\.([a-z_][a-z0-9_]*)")
_SCRIPT = re.compile(r"orchestration[\\/]+scripts[\\/]+([\w-]+\.py)")


def _textos(home: Path) -> list[str]:
    claude = home / ".claude"
    out = [p.read_text(encoding="utf-8", errors="ignore") for p in (claude / "hooks").glob("*.js")]
    for cfg, key in ((claude / "settings.json", "hooks"), (home / ".cursor" / "hooks.json", "hooks")):
        if cfg.exists():
            out.append(json.dumps(json.loads(cfg.read_text(encoding="utf-8")).get(key, {})))
    return out


def faltantes(home: Path, repo: Path = REPO) -> tuple[set[str], list[str]]:
    """(referencias de los hooks, las que no existen). Sigue a los scripts de
    orchestration/scripts/ que llaman los hooks y revisa tambien los modulos que importan."""
    textos = _textos(home)
    scripts = {s for t in textos for s in _SCRIPT.findall(t)}
    faltan = [f"scripts/{s}" for s in sorted(scripts) if not (repo / "scripts" / s).exists()]
    textos += [(repo / "scripts" / s).read_text(encoding="utf-8") for s in scripts if (repo / "scripts" / s).exists()]
    mods = {m for t in textos for m in _MOD.findall(t)}
    faltan += sorted(f"mmorch.{m}" for m in mods
                     if not (repo / "mmorch" / f"{m}.py").exists() and not (repo / "mmorch" / m).is_dir())
    return {f"scripts/{s}" for s in scripts} | {f"mmorch.{m}" for m in mods}, faltan


def test_hooks_de_esta_maquina_referencian_codigo_existente():
    refs, faltan = faltantes(Path.home())
    if not refs:
        pytest.skip("sin hooks que llamen a mmorch en esta maquina")
    assert not faltan, f"hooks llaman a codigo de mmorch inexistente: {faltan}"


def test_la_guarda_detecta_hooks_rotos(tmp_path):
    home, repo = tmp_path / "home", tmp_path / "repo"
    (home / ".claude" / "hooks").mkdir(parents=True)
    (home / ".cursor").mkdir()
    (repo / "mmorch").mkdir(parents=True)
    (repo / "scripts").mkdir()
    (repo / "mmorch" / "impacto.py").write_text("")
    (repo / "scripts" / "vivo.py").write_text("from mmorch.borrado import x\n")
    (home / ".claude" / "hooks" / "x.js").write_text('execFileSync(PY, ["-m", "mmorch.impacto"])')
    (home / ".claude" / "settings.json").write_text(json.dumps({"hooks": {"PreToolUse": [{"hooks": [
        {"command": r'"py.exe" "C:\u\orchestration\scripts\vivo.py"'}]}]}}))
    (home / ".cursor" / "hooks.json").write_text(json.dumps({"hooks": {"postToolUse": [
        {"command": r'"py.exe" "C:\u\orchestration\scripts\muerto.py" cursor'}]}}))
    refs, faltan = faltantes(home, repo)
    assert "mmorch.impacto" in refs
    assert faltan == ["scripts/muerto.py", "mmorch.borrado"]
