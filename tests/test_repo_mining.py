"""Tests de repo_mining — repo git local como 'remoto', fakes de LLM."""

import subprocess
from pathlib import Path

from mmorch.repo_mining import consume_queue, mine_repo


def make_fake_remote(tmp_path):
    src = tmp_path / "ajeno"
    src.mkdir()
    (src / "README.md").write_text("# Repo ajeno\nHace cosas.", encoding="utf-8")
    (src / "LICENSE").write_text("MIT License", encoding="utf-8")
    (src / "core.py").write_text("def util():\n    return 42\n", encoding="utf-8")
    for cmd in (["git", "init", "-b", "main"],
                ["git", "config", "user.email", "t@t"],
                ["git", "config", "user.name", "t"],
                ["git", "add", "-A"],
                ["git", "commit", "-m", "x", "--no-verify"]):
        subprocess.run(cmd, cwd=src, capture_output=True)
    return str(src)


def make_orch(tmp_path):
    orch = tmp_path / "orch"
    (orch / "logs").mkdir(parents=True)
    (orch / "vault" / "research").mkdir(parents=True)
    (orch / "vault" / "roadmaps").mkdir()
    (orch / "vault" / "roadmaps" / "candidatos.md").write_text(
        "# Candidatas\n\n## Vigentes\n\n## Archivadas\n", encoding="utf-8")
    return str(orch)


def fake_llm(prompt, schema):
    return {"resumen": "repo de utilidades", "licencia": "MIT",
            "grafts": [{"titulo": "util 42", "que": "helper universal",
                        "aplica_a": "orchestration",
                        "archivos_clave": ["core.py"], "esfuerzo": "trivial"},
                       {"titulo": "malo", "que": "vago",
                        "aplica_a": "nada"}]}


def test_mine_repo_distills_and_deletes(tmp_path):
    remote = make_fake_remote(tmp_path)
    orch = make_orch(tmp_path)
    r = mine_repo(remote, orch_root=orch, today="2026-08-19",
                  llm_fn=fake_llm,
                  verify_fn=lambda g: g["titulo"] == "util 42")  # refuta el vago
    assert r["ok"] and r["grafts"] == 2 and r["sobrevivieron"] == 1
    nota = Path(orch) / "vault" / "research" / "minado-ajeno-2026-08-19.md"
    text = nota.read_text(encoding="utf-8")
    assert "util 42" in text and "MIT" in text
    assert "malo" not in text                       # el refutado no persiste
    assert "core.py" in text                        # cita, no copia
    # candidata creada, parseable
    from mmorch.fuel import parse_candidatos
    md = (Path(orch) / "vault" / "roadmaps" / "candidatos.md").read_text(encoding="utf-8")
    cands = parse_candidatos(md)
    assert len(cands) == 1 and "util 42" in cands[0]["gist"]
    # la fruta se borro: ningun clon mmorch_mine_ vivo con contenido
    import glob
    import tempfile
    leftovers = [d for d in glob.glob(str(Path(tempfile.gettempdir()) / "mmorch_mine_*"))
                 if list(Path(d).rglob("core.py"))]
    assert leftovers == []


def test_consume_queue_one_per_night_and_comments(tmp_path):
    remote = make_fake_remote(tmp_path)
    orch = make_orch(tmp_path)
    q = Path(orch) / "logs" / "repos_queue.txt"
    q.write_text(f"# ya minado\n{remote}\notro-que-espera\n", encoding="utf-8")
    r = consume_queue(orch, today="2026-08-19", llm_fn=fake_llm,
                      verify_fn=lambda g: True)
    assert r["ok"]
    lines = q.read_text(encoding="utf-8").splitlines()
    assert lines[1].startswith("#")              # consumida -> comentada
    assert lines[2] == "otro-que-espera"         # la siguiente espera su noche


def test_consume_queue_empty_and_paused(tmp_path):
    orch = make_orch(tmp_path)
    assert consume_queue(orch, today="2026-08-19")["skipped"] == "sin cola"
    (Path(orch) / "logs" / "loop_paused").touch()
    assert consume_queue(orch, today="2026-08-19")["skipped"] == "paused"


def test_mine_repo_bad_url(tmp_path):
    orch = make_orch(tmp_path)
    r = mine_repo(str(tmp_path / "no-existe"), orch_root=orch,
                  today="2026-08-19", llm_fn=fake_llm)
    assert r["ok"] is False and "clone" in r["error"]
