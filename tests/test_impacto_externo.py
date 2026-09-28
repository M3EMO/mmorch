"""Lectores en prompts y configuracion de agentes (ticket 13): deteccion, bordes, cache y union."""
from mmorch import impacto as I
from mmorch import impacto_externo as E

WRITER = '"""Escribe logs/viejo.jsonl (docstring: no cuenta)."""\nimport os\n\nP = os.path.join("logs", "nightly.jsonl")\n'


def _home(tmp_path, files):
    for rel, body in files.items():
        p = tmp_path / "home" / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body, encoding="utf-8")
    return E.config_roots(tmp_path / "home")


def test_data_files_sin_docstrings():
    assert E.data_files(WRITER) == {"nightly.jsonl": 4}


def test_lista_prompts_y_hooks_con_linea(tmp_path):
    roots = _home(tmp_path, {
        ".claude/scheduled-tasks/resumen/SKILL.md": "---\nname: resumen\n---\nLeé logs/nightly.jsonl y avisá.\n",
        ".claude/hooks/gate.js": "// lee el estado\nconst f = 'nightly.jsonl';\n",
        ".claude/skills/otro/SKILL.md": "Leé old_nightly.jsonl y nightly.jsonl2\n"})   # otros archivos
    r = E.report(WRITER, "nightly.py", E.index(roots, tmp_path / "idx.json"))
    assert "- `nightly.jsonl` (linea 4) lo lee tambien: ~/.claude/scheduled-tasks/resumen/SKILL.md:4, " \
           "~/.claude/hooks/gate.js:2" in r
    assert "otro/SKILL.md" not in r and "viejo.jsonl" not in r


def test_settings_como_archivo_y_tope_de_cinco(tmp_path):
    files = {f".claude/skills/s{i}/SKILL.md": "nightly.jsonl\n" for i in range(6)}
    files[".claude/settings.json"] = '{"hooks": "nightly.jsonl"}\n'
    r = E.report(WRITER, "nightly.py", E.index(_home(tmp_path, files), tmp_path / "idx.json"))
    assert r.endswith("~/.claude/skills/s4/SKILL.md:1 y 2 mas")


def test_cache_no_relee_lo_que_no_cambio(tmp_path, monkeypatch):
    roots = _home(tmp_path, {".claude/skills/a/SKILL.md": "nightly.jsonl\n", ".claude/skills/b/SKILL.md": "x\n"})
    E.index(roots, tmp_path / "idx.json")
    orig, leidos = E._mentions, []
    monkeypatch.setattr(E, "_mentions", lambda text: leidos.append(text) or orig(text))
    E.index(roots, tmp_path / "idx.json")
    assert leidos == []
    (tmp_path / "home/.claude/skills/b/SKILL.md").write_text("nightly.jsonl otra vez\n", encoding="utf-8")
    idx = E.index(roots, tmp_path / "idx.json")
    assert leidos == ["nightly.jsonl otra vez\n"] and "b/SKILL.md:1" in E.report(WRITER, "n.py", idx)


def test_report_de_impacto_suma_la_seccion(tmp_path, monkeypatch):
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    roots = _home(tmp_path, {".claude/scheduled-tasks/r/SKILL.md": "Leé nightly.jsonl\n"})
    monkeypatch.setattr(E, "config_roots", lambda home=None: roots)
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "nightly.py").write_text(WRITER, encoding="utf-8")
    r = I.report(repo, repo / "nightly.py")
    assert r.startswith("LECTORES FUERA DEL REPO") and "scheduled-tasks/r/SKILL.md:1" in r
