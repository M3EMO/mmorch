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


def test_consume_queue_paralelo_marca_solo_los_que_salieron(tmp_path):
    """Mina en paralelo; el que falla queda SIN comentar (reintenta otra noche)."""
    remote = make_fake_remote(tmp_path)
    orch = make_orch(tmp_path)
    q = Path(orch) / "logs" / "repos_queue.txt"
    q.write_text(f"# ya minado\n{remote}\nurl-rota\n", encoding="utf-8")
    r = consume_queue(orch, today="2026-08-19", llm_fn=fake_llm,
                      verify_fn=lambda g: True, n=3)
    assert r["minados"] == 1 and len(r["resultados"]) == 2
    lines = q.read_text(encoding="utf-8").splitlines()
    assert lines[1].startswith("#")        # el bueno: consumido
    assert lines[2] == "url-rota"          # el roto: sigue en cola


def test_consume_queue_respeta_el_cupo(tmp_path):
    remote = make_fake_remote(tmp_path)
    orch = make_orch(tmp_path)
    q = Path(orch) / "logs" / "repos_queue.txt"
    q.write_text(f"{remote}\n{remote}\n{remote}\n", encoding="utf-8")
    r = consume_queue(orch, today="2026-08-19", llm_fn=fake_llm,
                      verify_fn=lambda g: True, n=2)
    assert len(r["resultados"]) == 2
    assert q.read_text(encoding="utf-8").splitlines()[2] == str(remote)


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


def _fake_search(items):
    """http_fn falso: siempre devuelve los mismos items."""
    return lambda url: {"items": items}


def test_discovery_alimenta_grafo_y_adopta_frontera(tmp_path):
    """La frontera (tema ajeno adyacente) entra como query y, si rinde, queda
    como interes permanente marcado '# auto'."""
    from mmorch.frontier import absorb
    from mmorch.repo_mining import discover_repos
    orch = Path(make_orch(tmp_path))
    (orch / "vault" / "roadmaps" / "roadmap.md").write_text(
        "**bandits contextuales**\n", encoding="utf-8")
    logs = str(orch / "logs")
    # grafo precargado: 'rlhf' es propio, 'reward-modeling' es su vecino ajeno
    absorb([{"topics": ["rlhf", "dpo"]}] * 4, logs_dir=logs, own=True)
    absorb([{"topics": ["dpo", "reward-modeling"]}] * 4, logs_dir=logs)

    item = {"html_url": "https://github.com/x/y", "topics": ["reward-modeling"],
            "license": {"spdx_id": "MIT"}}
    r = discover_repos(orch_root=str(orch), max_new=2,
                       http_fn=_fake_search([item]))

    assert "reward-modeling" in r["frontera"], r
    assert "reward-modeling" in r["adoptados"], r
    intereses = (orch / "vault" / "roadmaps" / "intereses.txt").read_text(
        encoding="utf-8")
    assert "reward-modeling" in intereses and "# auto" in intereses


def test_discovery_cooldown_retira_query_seca(tmp_path):
    """Query con 0 resultados dos veces seguidas descansa (presupuesto libre)."""
    from mmorch.repo_mining import discover_repos
    orch = Path(make_orch(tmp_path))
    (orch / "vault" / "roadmaps" / "roadmap.md").write_text(
        "**tema seco**\n", encoding="utf-8")
    kw = dict(orch_root=str(orch), max_new=2, http_fn=_fake_search([]))
    assert discover_repos(**kw)["queries"] == 1
    assert discover_repos(**kw)["queries"] == 1
    assert discover_repos(**kw).get("skipped")   # tercera: en cooldown


def test_collect_context_incluye_pdfs_del_repo(tmp_path):
    """Antes de docs_extract, ningun PDF entraba al contexto del juez —
    ahora un PDF suelto (whitepaper, docs/architecture.pdf) se lee via
    pypdfium2 (sin torch) y aparece en _collect_context."""
    from mmorch.repo_mining import _collect_context
    repo = tmp_path / "repo"
    repo.mkdir()
    stream = "BT /F1 14 Tf 20 100 Td (contenido del whitepaper mmorch) Tj ET"
    pdf_body = (
        "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n"
        "2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n"
        "3 0 obj<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>"
        "/MediaBox[0 0 300 150]/Contents 5 0 R>>endobj\n"
        "4 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n"
        f"5 0 obj<</Length {len(stream)}>>stream\n{stream}\nendstream\nendobj\n"
        "xref\n0 6\ntrailer<</Size 6/Root 1 0 R>>\nstartxref\n0\n%%EOF")
    (repo / "whitepaper.pdf").write_bytes(pdf_body.encode("latin-1"))
    ctx = _collect_context(repo)
    assert "contenido del whitepaper mmorch" in ctx
    assert "whitepaper.pdf" in ctx
