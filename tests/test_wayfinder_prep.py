"""Tests de wayfinder_prep (fakes, sin API — el invariante clave: JAMAS
responde el ticket, solo prepara el terreno)."""

from pathlib import Path

from mmorch.wayfinder_prep import _evidence, prep_map, prep_ticket


def make_orch(tmp_path):
    orch = tmp_path / "orch"
    (orch / "mmorch").mkdir(parents=True)
    (orch / "docs").mkdir()
    (orch / "vault" / "roadmaps").mkdir(parents=True)
    (orch / "mmorch" / "retention.py").write_text(
        "# decay ebbinghaus para memoria\nLAMBDA = 5e-10\n", encoding="utf-8")
    (orch / "mmorch" / "nada.py").write_text("x = 1\n", encoding="utf-8")
    return orch


def fake_llm(prompt, schema):
    return {"contexto": "hoy existe retention.py con decay",
            "opciones": [
                {"titulo": "grafo de citas", "como": "wikilinks", "costo": "2d",
                 "riesgo": "bajo"},
                {"titulo": "reescribir todo", "como": "big bang", "costo": "2sem",
                 "riesgo": "alto"}],
            "recomendacion": "grafo de citas primero",
            "pregunta_abierta": "¿cuánta memoria estás dispuesto a perder?"}


def test_evidence_encuentra_archivos_relevantes(tmp_path):
    orch = make_orch(tmp_path)
    ev = _evidence(orch, "¿cómo mejorar la memoria y su decay?")
    assert "retention.py" in ev and "ebbinghaus" in ev
    assert "nada.py" not in ev          # sin hits, no entra


def test_evidence_pregunta_sin_palabras_utiles(tmp_path):
    orch = make_orch(tmp_path)
    assert _evidence(orch, "¿esto?") == ""


def test_prep_ticket_marca_sospechosas_sin_borrarlas(tmp_path):
    orch = make_orch(tmp_path)
    d = prep_ticket("¿mejorar memoria?", orch_root=str(orch), llm_fn=fake_llm,
                    refute_fn=lambda o: o["riesgo"] == "alto")
    assert len(d["opciones"]) == 2      # la refutada NO se borra
    assert d["opciones"][0]["sospechosa"] is False
    assert d["opciones"][1]["sospechosa"] is True
    assert d["pregunta_abierta"]        # siempre queda algo solo-humano


def test_prep_map_escribe_dossier_sin_responder(tmp_path):
    orch = make_orch(tmp_path)
    path = prep_map("cerebro", ["¿mejorar memoria?", "¿qué módulo sigue?"],
                    orch_root=str(orch), llm_fn=fake_llm,
                    refute_fn=lambda o: False)
    text = Path(path).read_text(encoding="utf-8")
    assert "Ticket 1" in text and "Ticket 2" in text
    assert "SOLO VOS" in text                 # la pregunta abierta viaja
    assert "NO decisiones" in text            # el titulo lo dice explicito
    assert "decidir es tuyo" in text          # y el footer tambien
    assert (orch / ".scratch" / "cerebro" / "prep.md").exists()


def test_prep_map_marca_la_sospechosa_en_el_md(tmp_path):
    orch = make_orch(tmp_path)
    path = prep_map("m", ["¿mejorar memoria?"], orch_root=str(orch),
                    llm_fn=fake_llm, refute_fn=lambda o: o["riesgo"] == "alto")
    text = Path(path).read_text(encoding="utf-8")
    assert "sospechosa" in text and "reescribir todo" in text
