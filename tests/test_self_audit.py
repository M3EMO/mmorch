"""Tests de self_audit (fakes, sin engine ni API)."""

import json

from mmorch.self_audit import audit_global, audit_module, pick_module, run_one


def make_orch(tmp_path):
    orch = tmp_path / "orch"
    (orch / "logs").mkdir(parents=True)
    (orch / "mmorch").mkdir()
    (orch / "vault" / "research").mkdir(parents=True)
    (orch / "vault" / "roadmaps").mkdir()
    (orch / "vault" / "roadmaps" / "candidatos.md").write_text(
        "# Candidatas\n\n## Vigentes\n\n## Archivadas\n", encoding="utf-8")
    (orch / "mmorch" / "a.py").write_text("def f(): pass\n", encoding="utf-8")
    (orch / "mmorch" / "b.py").write_text("def g(): pass\n", encoding="utf-8")
    (orch / "mmorch" / "__init__.py").write_text("", encoding="utf-8")
    return orch


def fake_llm(prompt, schema):
    return {"resumen": "modulo chico, poca logica",
            "findings": [{"titulo": "relee disco en vez de usar memoria",
                          "severidad": "alta", "categoria": "estructural",
                          "detalle": "misma forma de bug que auto_repair"},
                         {"titulo": "acoplamiento", "severidad": "media",
                          "categoria": "bug", "detalle": "usa estado global X"},
                         {"titulo": "vago", "severidad": "baja",
                          "categoria": "otro", "detalle": "podria ser mas legible"}]}


def test_pick_module_prioriza_nunca_auditado_y_salta_dunder(tmp_path):
    orch = make_orch(tmp_path)
    assert pick_module(orch, {}, today="2026-08-19") in ("mmorch/a.py", "mmorch/b.py")
    state = {"mmorch/a.py": {"retry_after": "2026-09-19"}}
    assert pick_module(orch, state, today="2026-08-19") == "mmorch/b.py"
    # los dos ya vistos y en ventana -> None
    state2 = {"mmorch/a.py": {"retry_after": "2026-09-19"},
             "mmorch/b.py": {"retry_after": "2026-09-19"}}
    assert pick_module(orch, state2, today="2026-08-19") is None


def test_audit_module_persiste_nota_candidatas_y_log(tmp_path):
    orch = make_orch(tmp_path)
    r = audit_module("mmorch/a.py", orch_root=str(orch), today="2026-08-19",
                     llm_fn=fake_llm, verify_fn=lambda f: True)
    assert r["ok"] and r["findings"] == 3 and r["sobrevivieron"] == 3
    assert r["estructurales"] == 1
    notas = list((orch / "vault" / "research").glob("auditoria-*.md"))
    texto = notas[0].read_text(encoding="utf-8")
    assert len(notas) == 1 and "acoplamiento" in texto
    # el estructural va primero en la nota (mayor valor, orden explicito)
    assert texto.index("relee disco") < texto.index("acoplamiento")
    cand = (orch / "vault" / "roadmaps" / "candidatos.md").read_text(encoding="utf-8")
    assert "self-audit" in cand and "mmorch/a.py" in cand and "[estructural]" in cand
    log = json.loads((orch / "logs" / "self_audit.jsonl").read_text(
        encoding="utf-8").strip().splitlines()[-1])
    assert log["module"] == "mmorch/a.py" and log["sobrevivieron"] == 3
    assert log["estructurales"] == 1


def test_audit_module_refutador_filtra(tmp_path):
    orch = make_orch(tmp_path)
    r = audit_module("mmorch/a.py", orch_root=str(orch), today="2026-08-19",
                     llm_fn=fake_llm, verify_fn=lambda f: f["severidad"] != "baja")
    assert r["findings"] == 3 and r["sobrevivieron"] == 2


def test_run_one_rota_y_respeta_ventana(tmp_path):
    orch = make_orch(tmp_path)
    r1 = run_one(str(orch), today="2026-08-19", llm_fn=fake_llm, verify_fn=lambda f: True)
    r2 = run_one(str(orch), today="2026-08-19", llm_fn=fake_llm, verify_fn=lambda f: True)
    assert {r1["module"], r2["module"]} == {"mmorch/a.py", "mmorch/b.py"}
    r3 = run_one(str(orch), today="2026-08-19", llm_fn=fake_llm, verify_fn=lambda f: True)
    assert r3.get("skipped")


def test_run_one_respeta_pausa(tmp_path):
    orch = make_orch(tmp_path)
    (orch / "logs" / "loop_paused").touch()
    assert run_one(str(orch), today="2026-08-19")["skipped"] == "paused"


def test_audit_global_necesita_minimo_3_y_detecta_patron(tmp_path):
    orch = make_orch(tmp_path)
    assert audit_global(str(orch), today="2026-08-19")["skipped"]
    log_path = orch / "logs" / "self_audit.jsonl"
    entries = [
        {"fecha": "2026-08-15", "module": "mmorch/x.py", "sobrevivieron": 1,
         "estructurales": 1, "resumen": "relee disco en vez de usar estado en memoria"},
        {"fecha": "2026-08-16", "module": "mmorch/y.py", "sobrevivieron": 1,
         "estructurales": 1, "resumen": "relee disco en vez de usar estado en memoria"},
        {"fecha": "2026-08-17", "module": "mmorch/z.py", "sobrevivieron": 1,
         "estructurales": 1, "resumen": "relee disco en vez de usar estado en memoria"},
    ]
    log_path.write_text("\n".join(json.dumps(e) for e in entries) + "\n",
                        encoding="utf-8")

    seen = {}

    def fake_global(prompt, schema):
        seen["prompt"] = prompt
        return {"patrones_repetidos": ["x.py, y.py y z.py releen disco en vez "
                                       "de recibir estado en memoria"],
                "riesgo_principal": "el mismo bug se reinventa modulo a modulo",
                "recomendacion": "documentar la convencion"}

    r = audit_global(str(orch), today="2026-08-19", llm_fn=fake_global)
    assert "x.py" in seen["prompt"] and "y.py" in seen["prompt"]
    assert "ESTRUCTURALES" in seen["prompt"]
    assert r["patrones_repetidos"]
    saved = json.loads((orch / "logs" / "self_audit_global.jsonl").read_text(
        encoding="utf-8").strip().splitlines()[-1])
    assert saved["riesgo_principal"] == "el mismo bug se reinventa modulo a modulo"
