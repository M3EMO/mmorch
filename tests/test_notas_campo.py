"""scripts/notas_campo.py (orchestration-4rf): regla de elegibilidad de §5 y validador de fugas de §7.5, sin API."""
import importlib.util
import json
import os
import subprocess
from pathlib import Path

import pytest

_spec = importlib.util.spec_from_file_location("notas_campo", Path(__file__).resolve().parents[1] / "scripts" / "notas_campo.py")
NC = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(NC)


def _commit(repo: Path, files: dict, msg: str, t: int, base: str | None = None, task: str = "") -> str:
    for rel, txt in files.items():
        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
        (repo / rel).write_text(txt, encoding="utf-8")
    if base is not None:
        log = {"task": task or msg, "base_sha": base, "gate_rejects": [{"gate": "G3-compile", "detail": "x"}]}
        (repo / "docs/sdlc").mkdir(parents=True, exist_ok=True)
        (repo / "docs/sdlc/run-log.json").write_text(json.dumps(log), encoding="utf-8")
    env = {**os.environ, "GIT_AUTHOR_DATE": f"{1_700_000_000 + t} +0000", "GIT_COMMITTER_DATE": f"{1_700_000_000 + t} +0000"}
    subprocess.run(["git", "-C", repo, "add", "-A"], check=True)
    subprocess.run(["git", "-C", repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", msg], check=True, env=env)
    return subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()


@pytest.fixture
def repo(tmp_path):
    r = tmp_path / "r"
    r.mkdir()
    subprocess.run(["git", "init", "-q", "-b", "main", r], check=True)
    c0 = _commit(r, {"README": "x"}, "init", 1000)
    a = _commit(r, {"src/a.py": "a"}, "run A", 2000, base=c0)
    subprocess.run(["git", "-C", r, "checkout", "-qb", "sdlc/u"], check=True)
    u = _commit(r, {"src/u.py": "u"}, "run U sin merge", 3000, base=a)
    subprocess.run(["git", "-C", r, "checkout", "-q", "main"], check=True)
    i = _commit(r, {"src/i.py": "i", "tests/test_i.py": "t"}, "item I", 4000, base=a)
    later = _commit(r, {"src/z.py": "z"}, "run posterior", 5000, base=i)
    return str(r), {"c0": c0, "a": a, "u": u, "i": i, "later": later}


def test_elegibles_excluye_rama_sin_merge_y_corridas_posteriores(repo):
    r, c = repo
    runs = [{"repo": r, "commit": c[k]} for k in ("a", "u", "later")]
    elig = NC.elegibles({"id": "I", "repo": r, "commit": c["i"]}, runs)
    assert [e["commit"] for e in elig] == [c["a"]]


def test_main_escribe_bloque_y_sha_y_marca_items_sin_previas(repo, tmp_path):
    r, c = repo
    man = tmp_path / "m.json"
    man.write_text(json.dumps({"items": [{"id": "I", "repo": r, "commit": c["i"]}, {"id": "A", "repo": r, "commit": c["a"]}],
                               "runs": [{"repo": r, "commit": c["u"]}]}), encoding="utf-8")
    d = NC.main(str(man), str(tmp_path / "out"), dry=True)
    assert d["items"]["I"]["primaria"] and not d["items"]["A"]["primaria"]
    assert "run A" in (tmp_path / "out" / "I.txt").read_text(encoding="utf-8")
    assert (tmp_path / "out" / "notas.sha256").read_text().split()[0] == \
        __import__("hashlib").sha256((tmp_path / "out" / "notas.json").read_bytes()).hexdigest()


def test_validador_aborta_si_la_nota_nombra_un_archivo_que_el_item_agrega(tmp_path):
    r = tmp_path / "r"
    r.mkdir()
    subprocess.run(["git", "init", "-q", "-b", "main", r], check=True)
    c0 = _commit(r, {"README": "x"}, "init", 1000)
    a = _commit(r, {"src/a.py": "a"}, "run A", 2000, base=c0, task="preparar tests/test_i.py")
    i = _commit(r, {"tests/test_i.py": "t"}, "item I", 4000, base=a)
    man = tmp_path / "m.json"
    man.write_text(json.dumps({"items": [{"id": "I", "repo": str(r), "commit": i}], "runs": [{"repo": str(r), "commit": a}]}))
    with pytest.raises(SystemExit, match="test_i.py"):
        NC.main(str(man), str(tmp_path / "out"), dry=True)


def test_bloque_respeta_el_tope_y_prioriza_las_recientes():
    notas = ["a" * 4000, "b" * 4000, "c" * 4000]  # ~1000 tokens cada una
    assert NC.bloque(notas) == "a" * 4000
