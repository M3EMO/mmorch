"""orchestration-ior: `python -m sdlc bd` agrega reglas y hooks de bd por append, y `lint-md` chequea el contrato.

Corre el binario bd real con un HOME temporal. Casos: bloque viejo editado (reintegra), sin original en git (exit 2,
nada cambia), bloque intacto duplicado (mf-moe), misma edicion en los dos bloques (orchestration), seccion propia
existente, CLAUDE.md sin salto final. Criterio: bytes previos intactos, idempotente, `bd setup claude --check` pasa.
"""
import hashlib
import json
import pathlib
import shutil
import subprocess

import pytest

import sdlc.agentes as A

pytestmark = pytest.mark.skipif(shutil.which("bd") is None, reason="falta el binario bd")
REAL_SETTINGS = pathlib.Path.home() / ".claude" / "settings.json"


def _bloque(cuerpo: str) -> str:
    h = hashlib.sha256(cuerpo.strip().encode()).hexdigest()[:8]
    return f"<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:{h} -->\n{cuerpo}<!-- END BEADS INTEGRATION -->\n"


VIEJO = "## Beads viejo\n\n- Usa bd para todo.\n- Corre `bd prime`.\n"
EDITADO = "## Beads viejo\n\n- Usa bd para el backlog durable.\n- Corre `bd prime`.\n- Push = ASK, regla propia.\n"
PROPIAS = ["- Usa bd para el backlog durable.\n", "- Push = ASK, regla propia.\n"]


@pytest.fixture
def env(tmp_path, monkeypatch):
    home = tmp_path / "home"
    home.mkdir()
    monkeypatch.setenv("HOME", str(home))
    for k in ("GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME"):
        monkeypatch.setenv(k, "t")
    for k in ("GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL"):
        monkeypatch.setenv(k, "t@t")
    return tmp_path


def _git(repo, *args):
    return subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True).stdout


def _commit(repo, archivos: dict[str, str], msg: str):
    for rel, txt in archivos.items():
        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
        (repo / rel).write_text(txt, encoding="utf-8")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", msg)


def _repo(env, nombre="r"):
    repo = env / nombre
    repo.mkdir()
    _git(repo, "init", "-q")
    return repo


def _fotos(repo):
    return {p.relative_to(repo).as_posix(): p.read_bytes() for p in repo.rglob("*") if p.is_file()}


SETTINGS = {
    "permissions": {"allow": ["Bash(ls:*)"]},
    "hooks": {
        "PreToolUse": [{"matcher": "Edit", "hooks": [{"type": "command", "command": "echo mio"}]}],
        "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "bd prime"}]}],
        "PreCompact": [{"matcher": "", "hooks": [{"type": "command", "command": "bd prime"}]}],
    },
}


def test_bloque_viejo_editado_se_reintegra_sin_pisar_bytes(env):
    real = REAL_SETTINGS.read_bytes() if REAL_SETTINGS.exists() else None
    repo = _repo(env)
    _commit(repo, {"CLAUDE.md": "# Repo\n\nregla mia\n\n" + _bloque(VIEJO) + "\nmio abajo\n",
                   ".claude/settings.json": json.dumps(SETTINGS, indent=4) + "\n"}, "base")
    _commit(repo, {"CLAUDE.md": "# Repo\n\nregla mia\n\n" + _bloque(VIEJO).replace(VIEJO, EDITADO) + "\nmio abajo\n"}, "edito")
    hook = repo / ".git" / "hooks" / "pre-commit"
    hook.write_text("#!/bin/sh\necho USER-HOOK\n", encoding="utf-8")
    hook.chmod(0o755)
    claude_antes = (repo / "CLAUDE.md").read_text(encoding="utf-8")
    head = _git(repo, "rev-parse", "HEAD")

    r = A.bd_append(str(repo), hoy="2026-10-23")

    claude = (repo / "CLAUDE.md").read_text(encoding="utf-8")
    assert claude == "@AGENTS.md\n\n" + A.BLOQUE.sub("", claude_antes)  # import al inicio, resto intacto
    agents = (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert agents.startswith("# r\n") and "## Reglas propias de bd\n" in agents
    assert all(agents.count(x) == 1 for x in PROPIAS) and "reintegradas del bloque" in agents
    assert agents.index("END BEADS") < agents.index("## Reglas propias de bd\n")  # orden de D6
    plantilla = A.TPL_PROPIAS.read_text(encoding="utf-8")
    assert A.marcado("bd-propias", plantilla) in agents  # marcas de inicio y cierre, hash del interior
    assert "reintegradas" not in A.marcado("bd-propias", plantilla)  # lo reintegrado es del usuario, sin marcas
    assert len(A.BLOQUE.findall(agents)) == 1 and "BEADS" not in claude
    despues = json.loads((repo / ".claude" / "settings.json").read_text(encoding="utf-8"))
    assert A.sin_bd(despues) == A.sin_bd(SETTINGS)
    assert any(h["command"].startswith("bd prime") for g in despues["hooks"]["SessionStart"] for h in g["hooks"])
    assert "echo USER-HOOK" in hook.read_text(encoding="utf-8")
    assert subprocess.run(["bd", "setup", "claude", "--check"], cwd=repo, capture_output=True).returncode == 0
    assert _git(repo, "rev-parse", "HEAD") == head  # sin commit nuevo, aunque corrio bd init
    assert (repo / ".beads").is_dir() and r["reintegradas"]
    assert ".beads/config.yaml" in _git(repo, "diff", "--cached", "--name-only")  # bd init queda en stage

    fotos = _fotos(repo)
    assert A.bd_append(str(repo))["hecho"] == []
    assert _fotos(repo) == fotos  # idempotente
    assert (REAL_SETTINGS.read_bytes() if REAL_SETTINGS.exists() else None) == real
    assert any((env / "home").iterdir())  # bd uso el HOME temporal


def test_sin_original_en_git_sale_2_y_no_escribe(env):
    repo = _repo(env)
    _commit(repo, {"CLAUDE.md": "# Repo\n\n" + _bloque(VIEJO).replace(VIEJO, EDITADO)}, "base")
    fotos = _fotos(repo)
    with pytest.raises(A.Rechazo) as e:
        A.bd_append(str(repo))
    assert e.value.codigo == 2 and _fotos(repo) == fotos


def _bloque_actual(env) -> str:
    tmp = _repo(env, "gen")
    (tmp / "CLAUDE.md").write_text("@AGENTS.md\n", encoding="utf-8")
    (tmp / "AGENTS.md").write_text("# gen\n", encoding="utf-8")
    subprocess.run(["bd", "setup", "claude"], cwd=tmp, check=True, capture_output=True)
    return A.BLOQUE.search((tmp / "AGENTS.md").read_text(encoding="utf-8")).group(0)


def test_bloque_intacto_duplicado_queda_una_vez_en_agents(env):
    """mf-moe: el bloque actual esta en CLAUDE.md y en AGENTS.md."""
    actual = _bloque_actual(env)
    repo = _repo(env)
    _commit(repo, {"CLAUDE.md": "# mf\n\n- Convenciones.\n\n" + actual, "AGENTS.md": "# AGENTS\n\n" + actual}, "base")
    A.bd_append(str(repo))
    claude = (repo / "CLAUDE.md").read_text(encoding="utf-8")
    agents = (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert claude == "@AGENTS.md\n\n# mf\n\n- Convenciones.\n\n"
    assert agents.startswith("# AGENTS\n\n" + actual) and len(A.BLOQUE.findall(agents)) == 1


def test_misma_edicion_en_los_dos_bloques_se_reintegra_una_vez(env):
    """orchestration: CLAUDE.md y AGENTS.md traen el mismo bloque viejo con las mismas reglas propias."""
    repo = _repo(env)
    _commit(repo, {"CLAUDE.md": "# o\n\n" + _bloque(VIEJO), "AGENTS.md": "# o\n\n" + _bloque(VIEJO)}, "base")
    editado = _bloque(VIEJO).replace(VIEJO, EDITADO)
    solo_espacios = editado.replace("- Corre `bd prime`.", "-  Corre `bd prime`.")  # copia con espacios cambiados
    _commit(repo, {"CLAUDE.md": "# o\n\n" + editado, "AGENTS.md": "# o\n\n" + solo_espacios + "\n## SDLC\n\n- mio\n"}, "edito")
    A.bd_append(str(repo))
    agents = (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert all(agents.count(x) == 1 for x in PROPIAS) and "-  Corre" not in agents
    assert agents.index("BEGIN BEADS") < agents.index("## SDLC\n\n- mio\n")  # bd refresca en el mismo lugar


def test_seccion_propia_existente_y_claude_sin_salto_final(env):
    repo = _repo(env)
    propia = "# p\n\n## Reglas propias de bd\n\n- Adepor: regla hibrida.\n"
    _commit(repo, {"CLAUDE.md": "# p\n\nsin salto final", "AGENTS.md": propia}, "base")
    A.bd_append(str(repo))
    claude = (repo / "CLAUDE.md").read_text(encoding="utf-8")
    agents = (repo / "AGENTS.md").read_text(encoding="utf-8")
    assert claude == "@AGENTS.md\n\n# p\n\nsin salto final"
    assert agents.startswith(propia) and "mandan sobre el bloque" not in agents


CONTRATO = """# repo
North-star: medir algo con un oraculo.

## Comandos
- Tests: `pytest tests/ -q`

## Reglas
- Push = ASK.

## Hooks
- SessionStart: `bd prime --hook-json`

## Mapa
- Antes de cambiar el motor: lee `docs/agents/contexto.md`.
"""


def _contrato(tmp_path, agents=CONTRATO, claude="@AGENTS.md\n\n## Solo Claude\n- Ruteo: mmorch.\n", settings=None):
    (tmp_path / "AGENTS.md").write_text(agents, encoding="utf-8")
    (tmp_path / "CLAUDE.md").write_text(claude, encoding="utf-8")
    if settings is not None:
        (tmp_path / ".claude").mkdir()
        (tmp_path / ".claude" / "settings.json").write_text(json.dumps(settings), encoding="utf-8")
    return str(tmp_path)


def test_lint_contrato_valido(tmp_path):
    hooks = {"hooks": {"SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "bd prime --hook-json"}]}]}}
    assert A.lint(_contrato(tmp_path, settings=hooks)) == []


def test_lint_hallazgos(tmp_path):
    malo = CONTRATO.replace("## Reglas\n", "## Mapa0\n").replace("- Push = ASK.\n", "Esto es un parrafo largo.\n")
    malo = malo.replace("## Mapa\n", "## Reglas\n") + "- " + "x" * 201 + "\n"
    hooks = {"hooks": {"Stop": [{"matcher": "", "hooks": [{"type": "command", "command": "node aviso.js"}]}]}}
    h = "\n".join(A.lint(_contrato(tmp_path, agents=malo, claude="# sin import\n" + "- x\n" * 61, settings=hooks)))
    for esperado in ("parrafo", "203 caracteres", "fuera de orden", "falta `## Mapa`", "node aviso.js",
                     "CLAUDE.md: 62 lineas propias", "falta la linea @AGENTS.md"):
        assert esperado in h, esperado


def test_crlf_conserva_el_salto_del_archivo(env):
    """Clones del escritorio Windows: el comando no convierte un archivo CRLF a LF."""
    repo = _repo(env)
    (repo / "CLAUDE.md").write_bytes(b"# w\r\n\r\nregla\r\n")
    (repo / "AGENTS.md").write_bytes(b"# w\r\n\r\n- mio\r\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "base")
    A.bd_append(str(repo))
    assert (repo / "CLAUDE.md").read_bytes() == b"@AGENTS.md\r\n\r\n# w\r\n\r\nregla\r\n"
    agents = (repo / "AGENTS.md").read_bytes()
    assert agents.startswith(b"# w\r\n\r\n- mio\r\n") and agents.count(b"\n") == agents.count(b"\r\n")
    assert subprocess.run(["bd", "setup", "claude", "--check"], cwd=repo, capture_output=True).returncode == 0


@pytest.mark.parametrize("roto", [
    "<!-- BEGIN SDLC bd-propias v:1 hash:00000000 -->\n- mio sin cierre\n",
    "<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:00000000 -->\n- bd sin cierre\n",
    _bloque(VIEJO) + "\n" + _bloque(VIEJO),
])
def test_marcas_rotas_salen_2_y_no_escriben(env, roto):
    repo = _repo(env)
    _commit(repo, {"CLAUDE.md": "@AGENTS.md\n", "AGENTS.md": "# r\n\n" + roto}, "base")
    fotos = _fotos(repo)
    with pytest.raises(A.Rechazo) as e:
        A.bd_append(str(repo))
    assert e.value.codigo == 2 and _fotos(repo) == fotos
